import { describe, expect, it, vi } from 'vitest'
import {
  CancellationError,
  CancellationToken,
  CancellationTokenSource,
  throwIfCancelled,
} from './cancellation'

describe('cancellation', () => {
  it('exposes none and cancelled tokens', async () => {
    expect(CancellationToken.None.isCancellationRequested).toBe(false)
    const none = vi.fn()
    CancellationToken.None.onCancellationRequested(none).dispose()
    expect(none).not.toHaveBeenCalled()

    expect(CancellationToken.Cancelled.isCancellationRequested).toBe(true)
    const cancelled = vi.fn()
    const cancelledSubscription = CancellationToken.Cancelled.onCancellationRequested(cancelled)
    await Promise.resolve()
    expect(cancelled).toHaveBeenCalledOnce()
    cancelledSubscription.dispose()
  })

  it('cancels exactly once and supports late listeners', async () => {
    const source = new CancellationTokenSource()
    const listener = vi.fn()
    source.token.onCancellationRequested(listener)
    source.cancel()
    source.cancel()
    expect(listener).toHaveBeenCalledOnce()
    const late = vi.fn()
    source.token.onCancellationRequested(late)
    await Promise.resolve()
    expect(late).toHaveBeenCalledOnce()
    source.dispose()
  })

  it('inherits parent cancellation and handles an already-cancelled parent', () => {
    const parent = new CancellationTokenSource()
    const child = new CancellationTokenSource(parent.token)
    parent.cancel()
    expect(child.token.isCancellationRequested).toBe(true)
    child.dispose()

    const alreadyCancelled = new CancellationTokenSource(CancellationToken.Cancelled)
    expect(alreadyCancelled.token.isCancellationRequested).toBe(true)
    alreadyCancelled.dispose(true)
  })

  it('can cancel during disposal and throw typed errors', () => {
    const source = new CancellationTokenSource()
    source.dispose(true)
    expect(source.token.isCancellationRequested).toBe(true)
    expect(() => throwIfCancelled(source.token)).toThrow(CancellationError)
    expect(() => throwIfCancelled(CancellationToken.None)).not.toThrow()
    expect(new CancellationError('custom').message).toBe('custom')
  })
})
