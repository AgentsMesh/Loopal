import { describe, expect, it, vi } from 'vitest'
import { CancellationTokenSource } from './cancellation'
import { DeferredPromise, timeout } from './async'

describe('async helpers', () => {
  it('resolves and rejects deferred promises once', async () => {
    const resolved = new DeferredPromise<number>()
    resolved.resolve(4)
    resolved.resolve(5)
    resolved.reject(new Error('ignored'))
    await expect(resolved.promise).resolves.toBe(4)
    expect(resolved.isSettled).toBe(true)

    const rejected = new DeferredPromise<number>()
    const error = new Error('failed')
    rejected.reject(error)
    rejected.resolve(1)
    await expect(rejected.promise).rejects.toBe(error)
  })

  it('waits and cancels a timeout', async () => {
    vi.useFakeTimers()
    const completed = timeout(20)
    await vi.advanceTimersByTimeAsync(20)
    await expect(completed).resolves.toBeUndefined()

    const source = new CancellationTokenSource()
    const cancelled = timeout(50, source.token)
    source.cancel()
    await expect(cancelled).rejects.toThrow('Operation cancelled')

    await expect(timeout(10, source.token)).rejects.toThrow('Operation cancelled')
    vi.useRealTimers()
  })
})
