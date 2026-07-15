import { describe, expect, it, vi } from 'vitest'
import {
  Disposable,
  DisposableStore,
  MutableDisposable,
  combinedDisposable,
  disposeAll,
  isDisposable,
  toDisposable,
} from './lifecycle'

describe('lifecycle', () => {
  it('recognizes disposables', () => {
    expect(isDisposable({ dispose() {} })).toBe(true)
    expect(isDisposable({ dispose: 1 })).toBe(false)
    expect(isDisposable(null)).toBe(false)
    expect(isDisposable('dispose')).toBe(false)
  })

  it('creates idempotent disposables', () => {
    const callback = vi.fn()
    const disposable = toDisposable(callback)
    disposable.dispose()
    disposable.dispose()
    expect(callback).toHaveBeenCalledOnce()
  })

  it('disposes collections and reports one or many failures', () => {
    const first = vi.fn()
    disposeAll([toDisposable(first)])
    expect(first).toHaveBeenCalledOnce()

    const single = new Error('single')
    expect(() => disposeAll([{ dispose: () => { throw single } }])).toThrow(single)

    const failures = [new Error('one'), new Error('two')]
    expect(() =>
      disposeAll(failures.map((error) => ({ dispose: () => { throw error } }))),
    ).toThrow(AggregateError)
  })

  it('combines disposable resources', () => {
    const left = vi.fn()
    const right = vi.fn()
    combinedDisposable(toDisposable(left), toDisposable(right)).dispose()
    expect(left).toHaveBeenCalledOnce()
    expect(right).toHaveBeenCalledOnce()
  })

  it('owns, deletes, clears, and immediately disposes late resources', () => {
    const store = new DisposableStore()
    const deleted = vi.fn()
    const cleared = vi.fn()
    const deletedDisposable = store.add(toDisposable(deleted))
    store.add(toDisposable(cleared))
    store.delete(deletedDisposable)
    store.delete(deletedDisposable)
    expect(deleted).toHaveBeenCalledOnce()
    store.clear()
    expect(cleared).toHaveBeenCalledOnce()
    store.dispose()
    store.dispose()
    expect(store.isDisposed).toBe(true)
    const late = vi.fn()
    store.add(toDisposable(late))
    expect(late).toHaveBeenCalledOnce()
  })

  it('supports disposable subclasses', () => {
    const callback = vi.fn()
    class Resource extends Disposable {
      constructor() {
        super()
        this.register(toDisposable(callback))
      }
    }
    const resource = new Resource()
    resource.dispose()
    resource.dispose()
    expect(callback).toHaveBeenCalledOnce()
  })

  it('replaces and clears mutable disposables', () => {
    const holder = new MutableDisposable<{ dispose(): void }>()
    const first = { dispose: vi.fn() }
    const second = { dispose: vi.fn() }
    holder.value = first
    holder.value = first
    expect(holder.value).toBe(first)
    holder.value = second
    expect(first.dispose).toHaveBeenCalledOnce()
    holder.clear()
    expect(second.dispose).toHaveBeenCalledOnce()
    expect(holder.value).toBeUndefined()
    holder.dispose()
    holder.dispose()
    const late = { dispose: vi.fn() }
    holder.value = late
    expect(late.dispose).toHaveBeenCalledOnce()
  })
})
