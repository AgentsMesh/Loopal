import { type IDisposable } from '../../../../base/common/lifecycle'
import { type MetaHubStartupOptions } from '../../../desktop-host/node/host/desktop-host-types'
import { bindLocalMetaHub, type LocalMetaHubOperations, startLocalOnLaunch } from './loopal-local-metahub'
import { bindMetaHub } from './loopal-metahub-bind'
import { LoopalMetaHubCoordinator } from './loopal-metahub-coordinator'
import { type MetaHubOperations } from './loopal-metahub-service'
import { LoopalMetaHubSettings } from './loopal-metahub-settings'
import { type LoopalSessionDirectory } from '../sessions/loopal-session-directory'

export type MetaHubBackendOperations = MetaHubOperations & LocalMetaHubOperations

export class LoopalMetaHubRuntime implements IDisposable {
  private readonly settings: LoopalMetaHubSettings
  private readonly coordinator: LoopalMetaHubCoordinator

  constructor(options: {
    readonly binaryPath: string
    readonly parentPid: number
    readonly settingsPath?: string
  }) {
    this.settings = new LoopalMetaHubSettings(options.settingsPath)
    this.coordinator = new LoopalMetaHubCoordinator(options.binaryPath, options.parentPid)
  }

  get startup(): MetaHubStartupOptions | undefined {
    if (this.settings.publicValue.startLocalOnLaunch
      && this.coordinator.status.state !== 'running') return undefined
    return this.settings.startup
  }

  operations(
    directory: LoopalSessionDirectory,
    now: () => Date,
  ): MetaHubBackendOperations {
    return {
      ...bindMetaHub(this.settings, directory, now),
      ...bindLocalMetaHub(this.coordinator, this.settings),
    }
  }

  async load(): Promise<void> {
    await this.settings.load()
    await startLocalOnLaunch(this.coordinator, this.settings)
  }

  flush(): Promise<void> { return this.settings.flush() }
  stop(): Promise<void> { return this.coordinator.stop() }
  dispose(): void { this.coordinator.dispose() }
}
