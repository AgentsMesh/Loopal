import { Emitter, type Event } from '../../../base/common/event'
import { type IDisposable } from '../../../base/common/lifecycle'

export interface MessageTransport extends IDisposable {
  readonly onMessage: Event<unknown>
  send(message: unknown): void
}

export class MemoryTransport implements MessageTransport {
  private readonly messageEmitter = new Emitter<unknown>()
  private peer: MemoryTransport | undefined
  private disposed = false

  readonly onMessage = this.messageEmitter.event

  static pair(): readonly [MemoryTransport, MemoryTransport] {
    const left = new MemoryTransport()
    const right = new MemoryTransport()
    left.peer = right
    right.peer = left
    return [left, right]
  }

  send(message: unknown): void {
    if (this.disposed) {
      throw new Error('Transport is disposed')
    }
    const peer = this.peer
    if (!peer || peer.disposed) {
      throw new Error('Transport peer is unavailable')
    }
    queueMicrotask(() => peer.messageEmitter.fire(message))
  }

  dispose(): void {
    if (this.disposed) {
      return
    }
    this.disposed = true
    this.peer = undefined
    this.messageEmitter.dispose()
  }
}

interface PortEvent {
  readonly data: unknown
}

export interface MessagePortLike {
  postMessage(message: unknown): void
  start?(): void
  close?(): void
  addEventListener?(type: 'message', listener: (event: PortEvent) => void): void
  removeEventListener?(type: 'message', listener: (event: PortEvent) => void): void
  on?(type: 'message', listener: (event: PortEvent) => void): void
  off?(type: 'message', listener: (event: PortEvent) => void): void
}

export class MessagePortTransport implements MessageTransport {
  private readonly messageEmitter = new Emitter<unknown>()
  private disposed = false
  private readonly handleMessage = (event: PortEvent): void => {
    this.messageEmitter.fire(event.data)
  }

  readonly onMessage = this.messageEmitter.event

  constructor(private readonly port: MessagePortLike) {
    if (port.addEventListener) {
      port.addEventListener('message', this.handleMessage)
    } else {
      port.on?.('message', this.handleMessage)
    }
    port.start?.()
  }

  send(message: unknown): void {
    if (this.disposed) {
      throw new Error('Transport is disposed')
    }
    this.port.postMessage(message)
  }

  dispose(): void {
    if (this.disposed) {
      return
    }
    this.disposed = true
    if (this.port.removeEventListener) {
      this.port.removeEventListener('message', this.handleMessage)
    } else {
      this.port.off?.('message', this.handleMessage)
    }
    this.port.close?.()
    this.messageEmitter.dispose()
  }
}
