export interface TrailingInvalidation {
  invalidate(): Promise<void>
  dispose(): void
}

export function createTrailingInvalidation(
  run: (isCurrent: () => boolean) => Promise<void>,
): TrailingInvalidation {
  let dirty = false
  let disposed = false
  let active: Promise<void> | undefined

  const drain = async (): Promise<void> => {
    while (dirty && !disposed) {
      dirty = false
      await run(() => !disposed)
    }
  }

  return {
    invalidate(): Promise<void> {
      if (disposed) return Promise.resolve()
      dirty = true
      active ??= drain().finally(() => {
        active = undefined
        if (dirty && !disposed) void this.invalidate()
      })
      return active
    },
    dispose(): void {
      disposed = true
      dirty = false
    },
  }
}
