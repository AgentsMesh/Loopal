import { fireEvent, render, screen } from '@testing-library/react'
import { ConversationView } from './conversation-view'

afterEach(() => vi.unstubAllGlobals())

describe('ConversationView resize following', () => {
  it('keeps the tail visible when contextual panels resize the conversation', () => {
    let notify = (): void => undefined
    const disconnect = vi.fn()
    vi.stubGlobal('ResizeObserver', class {
      constructor(callback: ResizeObserverCallback) {
        notify = () => callback([], this as unknown as ResizeObserver)
      }
      observe(): void {}
      disconnect(): void { disconnect() }
    })
    const view = render(
      <div data-testid="scroll"><ConversationView entries={[]} /></div>,
    )
    const scroll = screen.getByTestId('scroll')
    Object.defineProperty(scroll, 'scrollHeight', { configurable: true, value: 500 })
    Object.defineProperty(scroll, 'clientHeight', { configurable: true, value: 100 })
    scroll.scrollTop = 400
    fireEvent.scroll(scroll)
    scroll.scrollTop = 350
    notify()
    expect(scroll.scrollTop).toBe(500)

    scroll.scrollTop = 0
    fireEvent.scroll(scroll)
    notify()
    expect(scroll.scrollTop).toBe(0)
    view.unmount()
    expect(disconnect).toHaveBeenCalledOnce()
  })
})
