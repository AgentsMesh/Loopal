import { type MetaHubRuntimeState } from '../../../../shared/contracts'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { loadMetaHubState, sameMetaHubState } from './loopal-metahub-projection'

export class LoopalMetaHubWatcher {
  private timer: ReturnType<typeof setTimeout> | undefined
  private active = false

  constructor(
    private readonly host: DesktopHostClient,
    private readonly now: () => Date,
    private readonly current: () => MetaHubRuntimeState | undefined,
    private readonly changed: () => Promise<void>,
  ) {}

  start(): void {
    if (this.active) return
    this.active = true
    this.schedule(0)
  }

  dispose(): void {
    this.active = false
    if (this.timer) clearTimeout(this.timer)
    this.timer = undefined
  }

  private schedule(delay: number): void {
    if (!this.active || this.timer) return
    this.timer = setTimeout(() => {
      this.timer = undefined
      void this.poll()
    }, delay)
    this.timer.unref?.()
  }

  private async poll(): Promise<void> {
    try {
      const state = await loadMetaHubState(this.host, this.now(), true)
      if (this.active && !sameMetaHubState(this.current(), state)) await this.changed()
    } catch {}
    finally { this.schedule(2_000) }
  }
}
