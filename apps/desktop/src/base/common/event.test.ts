import { describe, expect, it, vi } from 'vitest'
import { Emitter, Event } from './event'

describe('events', () => {
  it('subscribes, fires snapshots, and unsubscribes', () => {
    const first = vi.fn()
    const second = vi.fn()
    const emitter = new Emitter<number>()
    const firstSubscription = emitter.event(first)
    const secondSubscription = emitter.event((value) => {
      second(value)
      firstSubscription.dispose()
    })
    emitter.fire(7)
    emitter.fire(8)
    expect(first).toHaveBeenCalledTimes(1)
    expect(second).toHaveBeenCalledTimes(2)
    secondSubscription.dispose()
  })

  it('notifies listener lifecycle and disposal', () => {
    const first = vi.fn()
    const last = vi.fn()
    const emitter = new Emitter<void>({ onFirstListenerAdd: first, onLastListenerRemove: last })
    const left = emitter.event(() => undefined)
    const right = emitter.event(() => undefined)
    expect(first).toHaveBeenCalledOnce()
    left.dispose()
    expect(last).not.toHaveBeenCalled()
    right.dispose()
    expect(last).toHaveBeenCalledOnce()
    emitter.event(() => undefined)
    emitter.dispose()
    emitter.dispose()
    expect(last).toHaveBeenCalledTimes(2)
    expect(emitter.event(() => undefined)).toBeDefined()
    emitter.fire()
  })

  it('maps, filters, fires once, and provides a none event', () => {
    const emitter = new Emitter<number>()
    const values: string[] = []
    Event.filter(Event.map(emitter.event, (value) => `v${value}`), (value) => value !== 'v2')(
      (value) => values.push(value),
    )
    const once = vi.fn()
    Event.once(emitter.event)(once)
    Event.none<number>()(() => undefined).dispose()
    emitter.fire(1)
    emitter.fire(2)
    expect(values).toEqual(['v1'])
    expect(once).toHaveBeenCalledOnce()
  })

  it('handles a synchronously firing once source', () => {
    const listener = vi.fn()
    const dispose = vi.fn()
    Event.once<number>((callback) => {
      callback(3)
      return { dispose }
    })(listener)
    expect(listener).toHaveBeenCalledWith(3)
    expect(dispose).toHaveBeenCalledOnce()
  })

  it('disposes an asynchronously firing once source at the first value', () => {
    const source = new Emitter<number>()
    const listener = vi.fn()
    Event.once(source.event)(listener)
    source.fire(1)
    source.fire(2)
    expect(listener).toHaveBeenCalledOnce()
    expect(listener).toHaveBeenCalledWith(1)
  })

  it('routes listener errors to a handler or console', () => {
    const handled = vi.fn()
    const emitter = new Emitter<void>({ onListenerError: handled })
    emitter.event(() => { throw new Error('handled') })
    emitter.fire()
    expect(handled).toHaveBeenCalledOnce()

    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined)
    const fallback = new Emitter<void>()
    fallback.event(() => { throw new Error('fallback') })
    fallback.fire()
    expect(consoleError).toHaveBeenCalledOnce()
    fallback.dispose()
    fallback.event(() => undefined).dispose()
  })
})
