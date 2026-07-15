import { type IDisposable, toDisposable } from './lifecycle'

export type Listener<T> = (event: T) => void
export type Event<T> = (listener: Listener<T>) => IDisposable

export const Event = {
  none<T>(): Event<T> {
    return () => toDisposable(() => undefined)
  },

  once<T>(event: Event<T>): Event<T> {
    return (listener) => {
      let subscription: IDisposable | undefined
      let didFire = false
      subscription = event((value) => {
        if (didFire) {
          return
        }
        didFire = true
        subscription?.dispose()
        listener(value)
      })
      if (didFire) {
        subscription.dispose()
      }
      return subscription
    }
  },

  map<T, U>(event: Event<T>, mapper: (value: T) => U): Event<U> {
    return (listener) => event((value) => listener(mapper(value)))
  },

  filter<T>(event: Event<T>, predicate: (value: T) => boolean): Event<T> {
    return (listener) =>
      event((value) => {
        if (predicate(value)) {
          listener(value)
        }
      })
  },
}

export interface EmitterOptions {
  onFirstListenerAdd?: () => void
  onLastListenerRemove?: () => void
  onListenerError?: (error: unknown) => void
}

export class Emitter<T> implements IDisposable {
  private readonly listeners = new Set<Listener<T>>()
  private disposed = false
  private readonly options: EmitterOptions

  constructor(options: EmitterOptions = {}) {
    this.options = options
  }

  readonly event: Event<T> = (listener) => {
    if (this.disposed) {
      return toDisposable(() => undefined)
    }
    if (this.listeners.size === 0) {
      this.options.onFirstListenerAdd?.()
    }
    this.listeners.add(listener)
    return toDisposable(() => {
      const removed = this.listeners.delete(listener)
      if (removed && this.listeners.size === 0) {
        this.options.onLastListenerRemove?.()
      }
    })
  }

  fire(value: T): void {
    if (this.disposed) {
      return
    }
    for (const listener of [...this.listeners]) {
      try {
        listener(value)
      } catch (error) {
        if (this.options.onListenerError) {
          this.options.onListenerError(error)
        } else {
          console.error('Unhandled event listener error', error)
        }
      }
    }
  }

  dispose(): void {
    if (this.disposed) {
      return
    }
    this.disposed = true
    const hadListeners = this.listeners.size > 0
    this.listeners.clear()
    if (hadListeners) {
      this.options.onLastListenerRemove?.()
    }
  }
}
