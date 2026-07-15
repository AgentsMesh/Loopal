import { describe, expect, it, vi } from 'vitest'
import { createTrailingInvalidation } from './trailing-invalidation'

describe('createTrailingInvalidation', () => {
  it('runs one trailing cycle for invalidations during work', async () => {
    let release: (() => void) | undefined
    const first = new Promise<void>((resolve) => { release = resolve })
    const run = vi.fn().mockImplementationOnce(() => first).mockResolvedValue(undefined)
    const invalidation = createTrailingInvalidation(async () => run())
    const active = invalidation.invalidate()
    void invalidation.invalidate()
    void invalidation.invalidate()
    expect(run).toHaveBeenCalledOnce()
    release?.()
    await active
    expect(run).toHaveBeenCalledTimes(2)
  })

  it('restarts an invalidation queued at the drain/finally boundary', async () => {
    let invalidation!: ReturnType<typeof createTrailingInvalidation>
    let runs = 0
    invalidation = createTrailingInvalidation(async () => {
      runs += 1
      if (runs === 1) {
        queueMicrotask(() => queueMicrotask(() => void invalidation.invalidate()))
      }
    })
    await invalidation.invalidate()
    await vi.waitFor(() => expect(runs).toBe(2))
  })

  it('drops pending and future invalidations after disposal', async () => {
    let release: (() => void) | undefined
    const gate = new Promise<void>((resolve) => { release = resolve })
    const run = vi.fn(async () => gate)
    const invalidation = createTrailingInvalidation(run)
    const active = invalidation.invalidate()
    void invalidation.invalidate()
    invalidation.dispose()
    release?.()
    await active
    await invalidation.invalidate()
    expect(run).toHaveBeenCalledOnce()
  })
})
