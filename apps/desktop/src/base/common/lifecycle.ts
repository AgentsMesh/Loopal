export interface IDisposable {
  dispose(): void
}

export function isDisposable(value: unknown): value is IDisposable {
  return (
    typeof value === 'object' &&
    value !== null &&
    'dispose' in value &&
    typeof value.dispose === 'function'
  )
}

export function toDisposable(callback: () => void): IDisposable {
  let disposed = false
  return {
    dispose(): void {
      if (disposed) {
        return
      }
      disposed = true
      callback()
    },
  }
}

export function disposeAll(disposables: Iterable<IDisposable>): void {
  const errors: unknown[] = []
  for (const disposable of disposables) {
    try {
      disposable.dispose()
    } catch (error) {
      errors.push(error)
    }
  }
  if (errors.length === 1) {
    throw errors[0]
  }
  if (errors.length > 1) {
    throw new AggregateError(errors, 'Multiple errors occurred while disposing resources')
  }
}

export function combinedDisposable(...disposables: IDisposable[]): IDisposable {
  return toDisposable(() => disposeAll(disposables))
}

export class DisposableStore implements IDisposable {
  private readonly items = new Set<IDisposable>()
  private disposed = false

  get isDisposed(): boolean {
    return this.disposed
  }

  add<T extends IDisposable>(disposable: T): T {
    if (this.disposed) {
      disposable.dispose()
      return disposable
    }
    this.items.add(disposable)
    return disposable
  }

  delete(disposable: IDisposable): void {
    if (this.items.delete(disposable)) {
      disposable.dispose()
    }
  }

  clear(): void {
    const current = [...this.items]
    this.items.clear()
    disposeAll(current)
  }

  dispose(): void {
    if (this.disposed) {
      return
    }
    this.disposed = true
    this.clear()
  }
}

export abstract class Disposable implements IDisposable {
  private readonly store = new DisposableStore()

  protected register<T extends IDisposable>(disposable: T): T {
    return this.store.add(disposable)
  }

  dispose(): void {
    this.store.dispose()
  }
}

export class MutableDisposable<T extends IDisposable> implements IDisposable {
  private current: T | undefined
  private disposed = false

  get value(): T | undefined {
    return this.current
  }

  set value(next: T | undefined) {
    if (this.disposed) {
      next?.dispose()
      return
    }
    if (next === this.current) {
      return
    }
    this.current?.dispose()
    this.current = next
  }

  clear(): void {
    this.value = undefined
  }

  dispose(): void {
    if (this.disposed) {
      return
    }
    this.disposed = true
    this.current?.dispose()
    this.current = undefined
  }
}
