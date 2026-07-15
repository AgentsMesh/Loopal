import { createConnection, type Socket } from 'node:net'
import { Emitter, type Event } from '../../../../base/common/event'
import { Disposable, toDisposable } from '../../../../base/common/lifecycle'
import {
  isRecord,
  encodeJsonRpcFrame,
  JsonRpcFrameDecoder,
  JsonRpcRemoteError,
  MAX_DESKTOP_HOST_FRAME_BYTES,
  parseTcpAddress,
  type JsonRpcClientOptions,
  type JsonRpcNotification,
  type JsonRpcPendingRequest,
} from './jsonrpc-protocol'

export { JsonRpcRemoteError, type JsonRpcClientOptions, type JsonRpcNotification } from './jsonrpc-protocol'

export class JsonRpcClient extends Disposable {
  private readonly notifications = this.register(new Emitter<JsonRpcNotification>())
  private readonly closed = this.register(new Emitter<Error | undefined>())
  private readonly pending = new Map<number, JsonRpcPendingRequest>()
  private readonly requestTimeoutMs: number
  private readonly decoder: JsonRpcFrameDecoder
  private nextId = 1
  private didClose = false

  readonly onNotification: Event<JsonRpcNotification> = this.notifications.event
  readonly onClose: Event<Error | undefined> = this.closed.event

  constructor(private readonly socket: Socket, options: JsonRpcClientOptions = {}) {
    super()
    this.requestTimeoutMs = options.requestTimeoutMs ?? 10_000
    this.decoder = new JsonRpcFrameDecoder(options.maxFrameBytes ?? MAX_DESKTOP_HOST_FRAME_BYTES)
    socket.setEncoding('utf8')
    socket.on('data', this.acceptChunk)
    socket.once('close', this.acceptClose)
    socket.on('error', this.acceptError)
    this.register(
      toDisposable(() => {
        socket.off('data', this.acceptChunk)
        socket.off('close', this.acceptClose)
        socket.off('error', this.acceptError)
        socket.end()
        socket.destroy()
        this.finishClose(new Error('JSON-RPC client disposed'))
      }),
    )
  }

  static async connect(
    address: string,
    options: JsonRpcClientOptions & { readonly connectTimeoutMs?: number } = {},
  ): Promise<JsonRpcClient> {
    const { host, port } = parseTcpAddress(address)
    const socket = createConnection({ host, port })
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(() => {
        cleanup()
        socket.destroy()
        reject(new Error(`Timed out connecting to Loopal Hub at ${address}`))
      }, options.connectTimeoutMs ?? 5_000)
      const onConnect = (): void => {
        cleanup()
        resolve()
      }
      const onError = (error: Error): void => {
        cleanup()
        reject(error)
      }
      const cleanup = (): void => {
        clearTimeout(timer)
        socket.off('connect', onConnect)
        socket.off('error', onError)
      }
      socket.once('connect', onConnect)
      socket.once('error', onError)
    })
    return new JsonRpcClient(socket, options)
  }

  call(method: string, params: unknown = {}, signal?: AbortSignal): Promise<unknown> {
    if (this.didClose) {
      return Promise.reject(new Error('Loopal Hub connection is closed'))
    }
    if (signal?.aborted) {
      return Promise.reject(signal.reason ?? new Error('JSON-RPC request aborted'))
    }
    const id = this.nextId++
    return new Promise<unknown>((resolve, reject) => {
      const onAbort = (): void => {
        this.rejectPending(id, signal?.reason ?? new Error('JSON-RPC request aborted'))
      }
      signal?.addEventListener('abort', onAbort, { once: true })
      const timer = setTimeout(
        () => this.rejectPending(id, new Error(`Loopal Hub request timed out: ${method}`)),
        this.requestTimeoutMs,
      )
      this.pending.set(id, {
        resolve,
        reject,
        timer,
        removeAbortListener: () => signal?.removeEventListener('abort', onAbort),
      })
      try {
        const request = { jsonrpc: '2.0', id, method, params }
        this.socket.write(encodeJsonRpcFrame(request, this.decoder.maxFrameBytes))
      } catch (error) {
        this.rejectPending(id, error)
      }
    })
  }

  private readonly acceptChunk = (chunk: string | Buffer): void => {
    try {
      for (const line of this.decoder.accept(chunk)) {
        this.acceptMessage(line)
      }
    } catch (error) {
      this.socket.destroy(error instanceof Error ? error : new Error(String(error)))
    }
  }

  private acceptMessage(line: string): void {
    let message: unknown
    try {
      message = JSON.parse(line)
    } catch {
      this.socket.destroy(new Error('Loopal Hub sent malformed JSON'))
      return
    }
    if (!isRecord(message) || message.jsonrpc !== '2.0') {
      this.socket.destroy(new Error('Loopal Hub sent an invalid JSON-RPC message'))
      return
    }
    if (typeof message.method === 'string') {
      if (typeof message.id === 'number') {
        this.socket.write(
          `${JSON.stringify({
            jsonrpc: '2.0',
            id: message.id,
            error: { code: -32601, message: 'Desktop does not expose Hub-callable methods' },
          })}\n`,
        )
      } else {
        this.notifications.fire({ method: message.method, params: message.params })
      }
      return
    }
    if (typeof message.id !== 'number') {
      this.socket.destroy(new Error('Loopal Hub response is missing a numeric id'))
      return
    }
    const pending = this.takePending(message.id)
    if (!pending) {
      return
    }
    if (isRecord(message.error)) {
      pending.reject(
        new JsonRpcRemoteError(
          typeof message.error.code === 'number' ? message.error.code : -32603,
          typeof message.error.message === 'string' ? message.error.message : 'Remote error',
          message.error.data,
        ),
      )
    } else {
      pending.resolve(message.result)
    }
  }

  private rejectPending(id: number, reason: unknown): void {
    this.takePending(id)?.reject(reason)
  }

  private takePending(id: number): JsonRpcPendingRequest | undefined {
    const pending = this.pending.get(id)
    if (!pending) {
      return undefined
    }
    this.pending.delete(id)
    clearTimeout(pending.timer)
    pending.removeAbortListener()
    return pending
  }

  private readonly acceptError = (error: Error): void => this.finishClose(error)
  private readonly acceptClose = (): void => this.finishClose(undefined)

  private finishClose(reason: Error | undefined): void {
    if (this.didClose) {
      return
    }
    this.didClose = true
    const error = reason ?? new Error('Loopal Hub connection closed')
    for (const id of [...this.pending.keys()]) {
      this.rejectPending(id, error)
    }
    this.closed.fire(reason)
  }
}
