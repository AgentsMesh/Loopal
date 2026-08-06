import { spawn, type ChildProcess } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { mkdir } from 'node:fs/promises'
import { createConnection, type Socket } from 'node:net'
import { join } from 'node:path'
import { createInterface } from 'node:readline'
import { loopalBinaryPath, type DesktopFixture } from '../electron/electron-fixture'
import { isolatedTestEnvironment } from '../providers/mock-llm-fixture'

export interface MetaHubProcess {
  readonly child: ChildProcess
  readonly address: string
  readonly token: string
}

export interface RemoteHubProcess {
  readonly child: ChildProcess
  readonly probe: HubProbe
}

export async function startMetaHub(): Promise<MetaHubProcess> {
  const child = spawn(loopalBinaryPath(), [
    '--meta-hub', '127.0.0.1:0', '--meta-hub-parent-pid', String(process.pid),
  ], { env: isolatedTestEnvironment(), stdio: ['ignore', 'pipe', 'pipe'] })
  child.stderr?.resume()
  try {
    const value = await waitForPrefixedJson(child, 'LOOPAL_METAHUB ', 10_000)
    return { child, address: String(value.address), token: String(value.token) }
  } catch (error) {
    await stopProcess(child)
    throw error
  }
}

export async function startRemoteHub(
  desktop: DesktopFixture, meta: MetaHubProcess, name = 'hub-b',
): Promise<RemoteHubProcess> {
  const home = join(desktop.root, `${name}-home`)
  await mkdir(home, { recursive: true })
  const child = spawn(loopalBinaryPath(), [
    'desktop', 'serve', '--parent-pid', String(process.pid),
    '--join-hub', meta.address, '--hub-name', name,
  ], {
    cwd: desktop.project,
    env: isolatedTestEnvironment({
      HOME: home,
      LOOPAL_META_HUB_TOKEN: meta.token,
      ANTHROPIC_API_KEY: desktop.llm!.apiKey,
      ANTHROPIC_BASE_URL: desktop.llm!.baseUrl,
      LOOPAL_OTEL_ENABLED: 'false',
      LOOPAL_MCP_STARTUP_WAIT_SECS: '1',
    }),
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  child.stderr?.resume()
  try {
    const alive = await waitForDesktopReady(child)
    const probe = await HubProbe.connect(String(alive.addr), String(alive.token))
    return { child, probe }
  } catch (error) {
    await stopProcess(child)
    throw error
  }
}

export class HubProbe {
  private readonly pending = new Map<number, (message: Record<string, unknown>) => void>()
  private readonly observed: unknown[] = []
  private buffer = ''
  private nextId = 1

  private constructor(private readonly socket: Socket) {
    socket.setEncoding('utf8')
    socket.on('data', (chunk) => this.accept(String(chunk)))
  }

  static async connect(address: string, token: string): Promise<HubProbe> {
    const separator = address.lastIndexOf(':')
    const socket = createConnection({
      host: address.slice(0, separator), port: Number(address.slice(separator + 1)),
    })
    await new Promise<void>((resolve, reject) => {
      socket.once('connect', resolve)
      socket.once('error', reject)
    })
    const probe = new HubProbe(socket)
    await probe.call('hub/register', {
      name: `metahub-e2e-${randomUUID()}`, token, role: 'ui_client',
      capabilities: { permission: false, question: false, plan_approval: false },
    })
    return probe
  }

  notifications(): readonly unknown[] {
    return this.observed
  }

  async startModelTurn(text: string): Promise<void> {
    await this.call('hub/control', { target: 'main', command: { PermissionModeSwitch: 'bypass' } })
    await this.call('hub/route', {
      id: randomUUID(),
      source: 'Human',
      target: { hub: [], agent: 'main' },
      content: { text, images: [] },
      timestamp: new Date().toISOString(),
    })
  }

  close(): void {
    this.socket.end()
    this.socket.destroy()
  }

  private call(method: string, params: unknown): Promise<unknown> {
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      this.pending.set(id, (message) => message.error
        ? reject(new Error(`${method}: ${JSON.stringify(message.error)}`))
        : resolve(message.result))
      this.socket.write(`${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`)
    })
  }

  private accept(chunk: string): void {
    this.buffer += chunk
    let boundary = this.buffer.indexOf('\n')
    while (boundary >= 0) {
      const line = this.buffer.slice(0, boundary).replace(/\r$/, '')
      this.buffer = this.buffer.slice(boundary + 1)
      if (line) this.acceptMessage(JSON.parse(line) as Record<string, unknown>)
      boundary = this.buffer.indexOf('\n')
    }
  }

  private acceptMessage(message: Record<string, unknown>): void {
    if (typeof message.method === 'string' && message.id === undefined) {
      this.observed.push(message)
      return
    }
    if (typeof message.id === 'number') {
      const resolve = this.pending.get(message.id)
      this.pending.delete(message.id)
      resolve?.(message)
    }
  }
}

async function waitForDesktopReady(child: ChildProcess): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    const lines = createInterface({ input: child.stdout!, crlfDelay: Infinity })
    let alive: Record<string, unknown> | undefined
    const timer = setTimeout(() => finish(new Error('Timed out waiting for remote Hub')), 20_000)
    const finish = (error?: Error): void => {
      clearTimeout(timer); lines.close(); child.off('exit', exited)
      error ? reject(error) : resolve(alive!)
    }
    const exited = (): void => finish(new Error('Remote Hub exited before ready'))
    child.once('exit', exited)
    lines.on('line', (line) => {
      if (!line.startsWith('LOOPAL_DESKTOP ')) return
      try {
        const value = JSON.parse(line.slice(15)) as Record<string, unknown>
        if (value.phase === 'alive') alive = value
        if (value.phase === 'ready' && alive) finish()
      } catch { finish(new Error('Invalid remote Hub handshake')) }
    })
  })
}

async function waitForPrefixedJson(
  child: ChildProcess, prefix: string, timeout: number,
  accept: (value: Record<string, unknown>) => boolean = () => true,
): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    const lines = createInterface({ input: child.stdout!, crlfDelay: Infinity })
    const timer = setTimeout(() => finish(new Error(`Timed out waiting for ${prefix.trim()}`)), timeout)
    const finish = (error?: Error, value?: Record<string, unknown>): void => {
      clearTimeout(timer); lines.close(); child.off('exit', exited)
      error ? reject(error) : resolve(value!)
    }
    const exited = (): void => finish(new Error('Loopal fixture exited before handshake'))
    child.once('exit', exited)
    lines.on('line', (line) => {
      if (!line.startsWith(prefix)) return
      try {
        const value = JSON.parse(line.slice(prefix.length)) as Record<string, unknown>
        if (accept(value)) finish(undefined, value)
      } catch { finish(new Error('Invalid Loopal fixture handshake')) }
    })
  })
}

export async function stopProcess(child: ChildProcess): Promise<void> {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) return
  child.kill('SIGTERM')
  await new Promise<void>((resolve) => {
    const timer = setTimeout(() => { child.kill('SIGKILL'); resolve() }, 3_000)
    child.once('exit', () => { clearTimeout(timer); resolve() })
  })
}
