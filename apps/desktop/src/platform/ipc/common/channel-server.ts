import {
  CancellationTokenSource,
  type CancellationToken as CancellationTokenType,
} from '../../../base/common/cancellation'
import { type Event } from '../../../base/common/event'
import { DisposableStore, type IDisposable, toDisposable } from '../../../base/common/lifecycle'
import { type MessageTransport } from './transport'
import {
  RemoteError,
  WireMessageSchema,
  serializeError,
  type RequestMessage,
  type SubscribeMessage,
  type WireMessage,
} from './wire'

export interface ServerChannel<Context = unknown> {
  call(
    context: Context,
    command: string,
    arg: unknown,
    token: CancellationTokenType,
  ): Promise<unknown>
  listen?(context: Context, event: string, arg: unknown): Event<unknown>
}

export class ChannelServer<Context = unknown> implements IDisposable {
  private readonly channels = new Map<string, ServerChannel<Context>>()
  private readonly activeCalls = new Map<number, CancellationTokenSource>()
  private readonly subscriptions = new Map<number, IDisposable>()
  private readonly store = new DisposableStore()
  private disposed = false

  constructor(
    private readonly transport: MessageTransport,
    private readonly context: Context,
  ) {
    this.store.add(transport)
    this.store.add(transport.onMessage((raw) => void this.accept(raw)))
  }

  registerChannel(name: string, channel: ServerChannel<Context>): IDisposable {
    if (this.disposed) {
      throw new Error('Channel server is disposed')
    }
    if (this.channels.has(name)) {
      throw new Error(`Channel already registered: ${name}`)
    }
    this.channels.set(name, channel)
    return toDisposable(() => {
      this.channels.delete(name)
    })
  }

  dispose(): void {
    if (this.disposed) {
      return
    }
    this.disposed = true
    for (const source of this.activeCalls.values()) {
      source.dispose(true)
    }
    this.activeCalls.clear()
    for (const subscription of this.subscriptions.values()) {
      subscription.dispose()
    }
    this.subscriptions.clear()
    this.channels.clear()
    this.store.dispose()
  }

  private async accept(raw: unknown): Promise<void> {
    const parsed = WireMessageSchema.safeParse(raw)
    if (!parsed.success || this.disposed) {
      return
    }
    const message: WireMessage = parsed.data
    switch (message.type) {
      case 'request':
        await this.acceptRequest(message)
        break
      case 'cancel':
        this.activeCalls.get(message.id)?.cancel()
        break
      case 'subscribe':
        this.acceptSubscription(message)
        break
      case 'unsubscribe':
        this.subscriptions.get(message.id)?.dispose()
        this.subscriptions.delete(message.id)
        break
      default:
        break
    }
  }

  private async acceptRequest(message: RequestMessage): Promise<void> {
    const source = new CancellationTokenSource()
    this.activeCalls.set(message.id, source)
    try {
      const channel = this.requireChannel(message.channel)
      const result = await channel.call(this.context, message.command, message.arg, source.token)
      if (!this.disposed) {
        this.transport.send({ type: 'response', id: message.id, ok: true, result })
      }
    } catch (error) {
      if (!this.disposed) {
        this.transport.send({
          type: 'response',
          id: message.id,
          ok: false,
          error: serializeError(error),
        })
      }
    } finally {
      this.activeCalls.delete(message.id)
      source.dispose()
    }
  }

  private acceptSubscription(message: SubscribeMessage): void {
    try {
      const channel = this.requireChannel(message.channel)
      if (!channel.listen) {
        throw new RemoteError('EVENT_NOT_FOUND', `Channel has no events: ${message.channel}`)
      }
      const event = channel.listen(this.context, message.event, message.arg)
      const subscription = event((data) => {
        if (!this.disposed) {
          this.transport.send({ type: 'event', id: message.id, data })
        }
      })
      this.subscriptions.set(message.id, subscription)
    } catch (error) {
      this.transport.send({
        type: 'event',
        id: message.id,
        data: { error: serializeError(error) },
      })
    }
  }

  private requireChannel(name: string): ServerChannel<Context> {
    const channel = this.channels.get(name)
    if (!channel) {
      throw new RemoteError('CHANNEL_NOT_FOUND', `Unknown channel: ${name}`)
    }
    return channel
  }
}
