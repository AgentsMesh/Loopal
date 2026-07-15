import {
  CancellationError,
  CancellationToken,
  type CancellationToken as CancellationTokenType,
} from './cancellation'

export class DeferredPromise<T> {
  readonly promise: Promise<T>
  private resolvePromise!: (value: T | PromiseLike<T>) => void
  private rejectPromise!: (reason?: unknown) => void
  private settled = false

  constructor() {
    this.promise = new Promise<T>((resolve, reject) => {
      this.resolvePromise = resolve
      this.rejectPromise = reject
    })
  }

  get isSettled(): boolean {
    return this.settled
  }

  resolve(value: T | PromiseLike<T>): void {
    if (this.settled) {
      return
    }
    this.settled = true
    this.resolvePromise(value)
  }

  reject(reason?: unknown): void {
    if (this.settled) {
      return
    }
    this.settled = true
    this.rejectPromise(reason)
  }
}

export function timeout(
  milliseconds: number,
  token: CancellationTokenType = CancellationToken.None,
): Promise<void> {
  if (token.isCancellationRequested) {
    return Promise.reject(new CancellationError())
  }
  return new Promise<void>((resolve, reject) => {
    const handle = setTimeout(() => {
      subscription.dispose()
      resolve()
    }, milliseconds)
    const subscription = token.onCancellationRequested(() => {
      clearTimeout(handle)
      subscription.dispose()
      reject(new CancellationError())
    })
  })
}
