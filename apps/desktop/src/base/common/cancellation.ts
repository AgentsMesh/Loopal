import { Emitter, Event, type Event as EventType } from './event'
import { type IDisposable } from './lifecycle'

export class CancellationError extends Error {
  constructor(message = 'Operation cancelled') {
    super(message)
    this.name = 'CancellationError'
  }
}

export interface CancellationToken {
  readonly isCancellationRequested: boolean
  readonly onCancellationRequested: EventType<void>
}

const noneToken: CancellationToken = {
  isCancellationRequested: false,
  onCancellationRequested: Event.none(),
}

const cancelledToken: CancellationToken = {
  isCancellationRequested: true,
  onCancellationRequested: (listener) => {
    queueMicrotask(listener)
    return { dispose: () => undefined }
  },
}

export const CancellationToken = {
  None: noneToken,
  Cancelled: cancelledToken,
}

class MutableToken implements CancellationToken, IDisposable {
  private cancelled = false
  private emitter: Emitter<void> | undefined

  get isCancellationRequested(): boolean {
    return this.cancelled
  }

  get onCancellationRequested(): EventType<void> {
    if (this.cancelled) {
      return cancelledToken.onCancellationRequested
    }
    this.emitter ??= new Emitter<void>()
    return this.emitter.event
  }

  cancel(): void {
    if (this.cancelled) {
      return
    }
    this.cancelled = true
    this.emitter?.fire()
    this.emitter?.dispose()
    this.emitter = undefined
  }

  dispose(): void {
    this.emitter?.dispose()
    this.emitter = undefined
  }
}

export class CancellationTokenSource implements IDisposable {
  private readonly mutableToken = new MutableToken()
  private readonly parentSubscription: IDisposable | undefined

  constructor(parent?: CancellationToken) {
    if (parent?.isCancellationRequested) {
      this.parentSubscription = undefined
      this.mutableToken.cancel()
    } else {
      this.parentSubscription = parent?.onCancellationRequested(() => this.cancel())
    }
  }

  get token(): CancellationToken {
    return this.mutableToken
  }

  cancel(): void {
    this.mutableToken.cancel()
  }

  dispose(cancel = false): void {
    if (cancel) {
      this.cancel()
    }
    this.parentSubscription?.dispose()
    this.mutableToken.dispose()
  }
}

export function throwIfCancelled(token: CancellationToken): void {
  if (token.isCancellationRequested) {
    throw new CancellationError()
  }
}
