import {
  CancellationError,
  CancellationToken,
  type CancellationToken as CancellationTokenType,
} from '../../../base/common/cancellation'
import { Emitter, type Event } from '../../../base/common/event'
import { DisposableStore, type IDisposable, toDisposable } from '../../../base/common/lifecycle'
import { type MessageTransport } from './transport'
import {
  RemoteError,
  WireMessageSchema,
  type RequestMessage,
  type ResponseMessage,
  type SubscribeMessage,
} from './wire'

export interface ChannelClient extends IDisposable {
  call<T>(
    channel: string,
    command: string,
    arg?: unknown,
    token?: CancellationTokenType,
  ): Promise<T>
  listen<T>(channel: string, event: string, arg?: unknown): Event<T>
}

interface PendingCall {
  readonly resolve: (value: unknown) => void
  readonly reject: (reason: unknown) => void
  readonly cancellation: IDisposable
}

export class ChannelClientImpl implements ChannelClient {
  private readonly store = new DisposableStore()
  private readonly pending = new Map<number, PendingCall>()
  private readonly subscriptions = new Map<number, Emitter<unknown>>()
  private nextId = 1
  private disposed = false

  constructor(private readonly transport: MessageTransport) {
    this.store.add(transport)
    this.store.add(transport.onMessage((raw) => this.accept(raw)))
  }

  call<T>(
    channel: string,
    command: string,
    arg?: unknown,
    token: CancellationTokenType = CancellationToken.None,
  ): Promise<T> {
    if (this.disposed) {
      return Promise.reject(new Error('Channel client is disposed'))
    }
    if (token.isCancellationRequested) {
      return Promise.reject(new CancellationError())
    }
    const id = this.nextId++
    return new Promise<T>((resolve, reject) => {
      const cancellation = token.onCancellationRequested(() => {
        this.transport.send({ type: 'cancel', id })
        this.pending.delete(id)
        reject(new CancellationError())
      })
      this.pending.set(id, {
        resolve: (value) => resolve(value as T),
        reject,
        cancellation,
      })
      const message: RequestMessage = { type: 'request', id, channel, command }
      this.transport.send(arg === undefined ? message : { ...message, arg })
    })
  }

  listen<T>(channel: string, event: string, arg?: unknown): Event<T> {
    return (listener) => {
      if (this.disposed) {
        return toDisposable(() => undefined)
      }
      const id = this.nextId++
      const emitter = new Emitter<unknown>()
      this.subscriptions.set(id, emitter)
      const local = emitter.event((value) => listener(value as T))
      const message: SubscribeMessage = { type: 'subscribe', id, channel, event }
      this.transport.send(arg === undefined ? message : { ...message, arg })
      return toDisposable(() => {
        local.dispose()
        emitter.dispose()
        if (this.subscriptions.delete(id)) {
          this.transport.send({ type: 'unsubscribe', id })
        }
      })
    }
  }

  dispose(): void {
    if (this.disposed) {
      return
    }
    this.disposed = true
    for (const pending of this.pending.values()) {
      pending.cancellation.dispose()
      pending.reject(new Error('Channel client disposed'))
    }
    this.pending.clear()
    for (const emitter of this.subscriptions.values()) {
      emitter.dispose()
    }
    this.subscriptions.clear()
    this.store.dispose()
  }

  private accept(raw: unknown): void {
    const parsed = WireMessageSchema.safeParse(raw)
    if (!parsed.success) {
      return
    }
    const message = parsed.data
    if (message.type === 'response') {
      this.acceptResponse(message)
    } else if (message.type === 'event') {
      this.subscriptions.get(message.id)?.fire(message.data)
    }
  }

  private acceptResponse(message: ResponseMessage): void {
    const pending = this.pending.get(message.id)
    if (!pending) {
      return
    }
    this.pending.delete(message.id)
    pending.cancellation.dispose()
    if (message.ok) {
      pending.resolve(message.result)
    } else {
      pending.reject(new RemoteError(message.error.code, message.error.message, message.error.data))
    }
  }
}
