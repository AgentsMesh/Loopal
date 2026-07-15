import { DeferredPromise } from '../../../../base/common/async'

export interface DesktopHostOperation<T> {
  readonly command: number
  readonly promise: Promise<T>
}

export interface PendingDesktopHostOperation<T> {
  readonly operation: DesktopHostOperation<T>
  complete(task: Promise<T>): void
}

export function createOperation<T>(command: number): PendingDesktopHostOperation<T> {
  const result = new DeferredPromise<T>()
  return {
    operation: { command, promise: result.promise },
    complete(task): void {
      void task.then(
        (value) => result.resolve(value),
        (error) => result.reject(error),
      )
    },
  }
}
