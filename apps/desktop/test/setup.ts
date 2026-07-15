import '@testing-library/jest-dom/vitest'

class TestResizeObserver implements ResizeObserver {
  constructor(private readonly callback: ResizeObserverCallback) {}
  observe(target: Element): void { this.callback([], this); void target }
  unobserve(): void {}
  disconnect(): void {}
}

vi.stubGlobal('ResizeObserver', TestResizeObserver)
vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => (
  window.setTimeout(() => callback(performance.now()), 0)
))
vi.stubGlobal('cancelAnimationFrame', (id: number) => window.clearTimeout(id))
