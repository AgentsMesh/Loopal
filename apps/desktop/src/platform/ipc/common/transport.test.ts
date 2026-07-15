import { describe, expect, it, vi } from 'vitest'
import { MemoryTransport, MessagePortTransport, type MessagePortLike } from './transport'

describe('IPC transports', () => {
  it('pairs memory transports and enforces lifecycle', async () => {
    const [left, right] = MemoryTransport.pair()
    const listener = vi.fn()
    right.onMessage(listener)
    left.send({ value: 1 })
    await Promise.resolve()
    expect(listener).toHaveBeenCalledWith({ value: 1 })
    right.dispose()
    right.dispose()
    expect(() => left.send('late')).toThrow('unavailable')
    left.dispose()
    expect(() => left.send('disposed')).toThrow('disposed')
  })

  it('adapts DOM-style message ports', () => {
    let listener: ((event: { data: unknown }) => void) | undefined
    const port: MessagePortLike = {
      postMessage: vi.fn(),
      start: vi.fn(),
      close: vi.fn(),
      addEventListener: vi.fn((_type, next) => { listener = next }),
      removeEventListener: vi.fn(),
    }
    const transport = new MessagePortTransport(port)
    const received = vi.fn()
    transport.onMessage(received)
    listener?.({ data: 'hello' })
    expect(received).toHaveBeenCalledWith('hello')
    transport.send('world')
    expect(port.postMessage).toHaveBeenCalledWith('world')
    expect(port.start).toHaveBeenCalledOnce()
    transport.dispose()
    transport.dispose()
    expect(port.removeEventListener).toHaveBeenCalled()
    expect(port.close).toHaveBeenCalledOnce()
    expect(() => transport.send('late')).toThrow('disposed')
  })

  it('adapts event-emitter-style message ports', () => {
    let listener: ((event: { data: unknown }) => void) | undefined
    const port: MessagePortLike = {
      postMessage: vi.fn(),
      on: vi.fn((_type, next) => { listener = next }),
      off: vi.fn(),
    }
    const transport = new MessagePortTransport(port)
    const received = vi.fn()
    transport.onMessage(received)
    listener?.({ data: 9 })
    expect(received).toHaveBeenCalledWith(9)
    transport.dispose()
    expect(port.off).toHaveBeenCalled()
  })
})
