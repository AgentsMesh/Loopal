import { randomUUID } from 'node:crypto'
import { createConnection, type Socket } from 'node:net'

const RPC_TIMEOUT_MS = 5_000

interface PendingCall {
  readonly resolve: (value: unknown) => void
  readonly reject: (error: Error) => void
  readonly timer: NodeJS.Timeout
}

export class HubProbe {
  private readonly pending = new Map<number, PendingCall>()
  private readonly observed: unknown[] = []
  private buffer = ''
  private nextId = 1
  private closed = false

  private constructor(private readonly socket: Socket) {
    socket.setEncoding('utf8')
    socket.on('data', this.onData)
    socket.on('error', this.onError)
    socket.on('close', this.onClose)
  }

  static async connect(address: string, token: string): Promise<HubProbe> {
    const separator = address.lastIndexOf(':')
    const socket = createConnection({
      host: address.slice(0, separator), port: Number(address.slice(separator + 1)),
    })
    let probe: HubProbe | undefined
    try {
      await waitForSocket(socket)
      probe = new HubProbe(socket)
      await probe.call('hub/register', {
        name: `metahub-e2e-${randomUUID()}`, token, role: 'ui_client',
        capabilities: { permission: true, question: true, plan_approval: true },
      })
      return probe
    } catch (error) {
      probe?.close()
      socket.destroy()
      throw error
    }
  }

  notifications(): readonly unknown[] {
    return this.observed
  }

  async startModelTurn(text: string): Promise<void> {
    await this.call('hub/control', { target: 'main', command: { PermissionModeSwitch: 'bypass' } })
    await this.call('hub/route', {
      id: randomUUID(), source: 'Human', target: { hub: [], agent: 'main' },
      content: { text, images: [] }, timestamp: new Date().toISOString(),
    })
  }

  close(): void {
    this.dispose(new Error('Hub probe closed'))
  }

  private call(method: string, params: unknown): Promise<unknown> {
    if (this.closed) return Promise.reject(new Error(`Hub probe is closed: ${method}`))
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.dispose(new Error(`${method} timed out after ${RPC_TIMEOUT_MS}ms`))
      }, RPC_TIMEOUT_MS)
      this.pending.set(id, { resolve, reject, timer })
      this.socket.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`, (error) => {
        if (error) this.dispose(error)
      })
    })
  }

  private readonly onData = (chunk: Buffer | string): void => this.accept(String(chunk))
  private readonly onError = (error: Error): void => this.dispose(error)
  private readonly onClose = (): void => this.dispose(new Error('Hub probe transport closed'))

  private accept(chunk: string): void {
    this.buffer += chunk
    let boundary = this.buffer.indexOf('\n')
    while (boundary >= 0) {
      const line = this.buffer.slice(0, boundary).replace(/\r$/, '')
      this.buffer = this.buffer.slice(boundary + 1)
      try {
        if (line) this.acceptMessage(JSON.parse(line) as Record<string, unknown>)
      } catch (error) {
        this.dispose(error instanceof Error ? error : new Error(String(error)))
        return
      }
      boundary = this.buffer.indexOf('\n')
    }
  }

  private acceptMessage(message: Record<string, unknown>): void {
    if (typeof message.method === 'string' && message.id === undefined) {
      this.observed.push(message)
      return
    }
    if (typeof message.id !== 'number') return
    const pending = this.pending.get(message.id)
    if (!pending) return
    this.pending.delete(message.id)
    clearTimeout(pending.timer)
    if (message.error) pending.reject(new Error(JSON.stringify(message.error)))
    else pending.resolve(message.result)
  }

  private dispose(error: Error): void {
    if (this.closed) return
    this.closed = true
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timer)
      pending.reject(error)
    }
    this.pending.clear()
    this.socket.off('data', this.onData)
    this.socket.off('error', this.onError)
    this.socket.off('close', this.onClose)
    this.socket.destroy()
  }
}

function waitForSocket(socket: Socket): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => finish(new Error('Timed out connecting capable UI probe')), RPC_TIMEOUT_MS)
    const finish = (error?: Error): void => {
      clearTimeout(timer)
      socket.off('connect', onConnect)
      socket.off('error', onError)
      error ? reject(error) : resolve()
    }
    const onConnect = (): void => finish()
    const onError = (error: Error): void => finish(error)
    socket.once('connect', onConnect)
    socket.once('error', onError)
  })
}
