import { CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  type JoinMetaHubInput,
  type MetaHubRuntimeState,
  type MetaHubRuntimeTarget,
  type MetaHubSettings,
  type UpdateMetaHubSettingsInput,
} from '../../../../shared/contracts'
import {
  invalidateMetaHubState,
  loadMetaHubState,
} from './loopal-metahub-projection'
import { type LoopalMetaHubSettings } from './loopal-metahub-settings'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

export interface MetaHubOperations {
  getMetaHubSettings(token?: CancellationToken): Promise<MetaHubSettings>
  updateMetaHubSettings(
    input: UpdateMetaHubSettingsInput, token?: CancellationToken,
  ): Promise<MetaHubSettings>
  getMetaHubStatus(
    target: MetaHubRuntimeTarget, token?: CancellationToken,
  ): Promise<MetaHubRuntimeState>
  joinMetaHub(input: JoinMetaHubInput, token?: CancellationToken): Promise<MetaHubRuntimeState>
  disconnectMetaHub(
    target: MetaHubRuntimeTarget, token?: CancellationToken,
  ): Promise<MetaHubRuntimeState>
}

interface MetaHubServiceOptions {
  readonly settings: LoopalMetaHubSettings
  readonly runtime: (target: MetaHubRuntimeTarget) => SessionRuntimeHandle
  readonly resync: (sessionId: string) => Promise<void>
  readonly now: () => Date
}

export class LoopalMetaHubService implements MetaHubOperations {
  constructor(private readonly options: MetaHubServiceOptions) {}

  async getMetaHubSettings(token = CancellationToken.None): Promise<MetaHubSettings> {
    throwIfCancelled(token)
    return this.options.settings.publicValue
  }

  async updateMetaHubSettings(
    input: UpdateMetaHubSettingsInput,
    token = CancellationToken.None,
  ): Promise<MetaHubSettings> {
    throwIfCancelled(token)
    const value = await this.options.settings.update(input)
    throwIfCancelled(token)
    return value
  }

  async getMetaHubStatus(
    target: MetaHubRuntimeTarget,
    token = CancellationToken.None,
  ): Promise<MetaHubRuntimeState> {
    throwIfCancelled(token)
    const runtime = this.options.runtime(target)
    const value = await loadMetaHubState(runtime.host, this.options.now(), true)
    throwIfCancelled(token)
    return value
  }

  async joinMetaHub(
    input: JoinMetaHubInput,
    token = CancellationToken.None,
  ): Promise<MetaHubRuntimeState> {
    throwIfCancelled(token)
    const runtime = this.options.runtime(input)
    const stored = this.options.settings.credentials
    const address = input.address ?? stored?.address
    const hubName = input.hubName ?? stored?.hubName
    const secret = input.token ?? stored?.token
    if (!address || !hubName || !secret) {
      throw new Error('MetaHub address, hub name, and token are required')
    }
    const current = await loadMetaHubState(runtime.host, this.options.now(), true)
    if (current.state !== 'disconnected') {
      await call(runtime, 'hub/leave_meta', {}, token)
    }
    throwIfCancelled(token)
    await call(runtime, 'hub/join_meta', {
      address, hub_name: hubName, token: secret,
    }, token)
    invalidateMetaHubState(runtime.host)
    const value = await loadMetaHubState(runtime.host, this.options.now(), true)
    await this.options.resync(input.sessionId)
    return value
  }

  async disconnectMetaHub(
    target: MetaHubRuntimeTarget,
    token = CancellationToken.None,
  ): Promise<MetaHubRuntimeState> {
    throwIfCancelled(token)
    const runtime = this.options.runtime(target)
    await call(runtime, 'hub/leave_meta', {}, token)
    invalidateMetaHubState(runtime.host)
    const value = await loadMetaHubState(runtime.host, this.options.now(), true)
    await this.options.resync(target.sessionId)
    return value
  }
}

async function call(
  runtime: SessionRuntimeHandle,
  method: string,
  params: unknown,
  token: CancellationToken,
): Promise<unknown> {
  throwIfCancelled(token)
  const controller = new AbortController()
  const subscription = token.onCancellationRequested(() => controller.abort())
  try {
    const result = await runtime.host.request(method, params, controller.signal)
    throwIfCancelled(token)
    return result
  } finally {
    subscription.dispose()
  }
}
