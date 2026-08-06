type RefreshSnapshot = (emit: boolean) => Promise<void>

export class LoopalSessionRefresh {
  private running: Promise<void> | undefined
  private pending = false
  private shouldEmit = false
  private timer: ReturnType<typeof setTimeout> | undefined
  private active = true

  constructor(private readonly refreshSnapshot: RefreshSnapshot) {}

  request(emit: boolean): Promise<void> {
    if (!this.active) return Promise.resolve()
    this.pending = true
    this.shouldEmit ||= emit
    this.running ??= this.drain().finally(() => {
      this.running = undefined
      if (this.pending && this.active) void this.request(false).catch(() => undefined)
    })
    return this.running
  }

  invalidate(): void {
    if (this.timer || !this.active) return
    this.timer = setTimeout(() => {
      this.timer = undefined
      void this.request(true).catch(() => undefined)
    }, 16)
  }

  requestImmediately(): void {
    if (this.timer) clearTimeout(this.timer)
    this.timer = undefined
    void this.request(true).catch(() => undefined)
  }

  dispose(): void {
    this.active = false
    if (this.timer) clearTimeout(this.timer)
    this.timer = undefined
  }

  private async drain(): Promise<void> {
    while (this.pending && this.active) {
      this.pending = false
      const emit = this.shouldEmit
      this.shouldEmit = false
      await this.refreshSnapshot(emit)
    }
  }
}
