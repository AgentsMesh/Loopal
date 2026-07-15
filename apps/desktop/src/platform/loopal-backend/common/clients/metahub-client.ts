import {
  LocalMetaHubStatusSchema,
  MetaHubRuntimeStateSchema,
  MetaHubSettingsSchema,
  type JoinMetaHubInput,
  type LocalMetaHubStatus,
  type MetaHubRuntimeState,
  type MetaHubRuntimeTarget,
  type MetaHubSettings,
  type StartLocalMetaHubInput,
  type UpdateMetaHubSettingsInput,
} from '../../../../shared/contracts'
import { type ChannelClient } from '../../../ipc/common/channel'

export interface MetaHubClientOperations {
  getMetaHubSettings(): Promise<MetaHubSettings>
  updateMetaHubSettings(input: UpdateMetaHubSettingsInput): Promise<MetaHubSettings>
  getMetaHubStatus(target: MetaHubRuntimeTarget): Promise<MetaHubRuntimeState>
  joinMetaHub(input: JoinMetaHubInput): Promise<MetaHubRuntimeState>
  disconnectMetaHub(target: MetaHubRuntimeTarget): Promise<MetaHubRuntimeState>
  getLocalMetaHubStatus(): Promise<LocalMetaHubStatus>
  startLocalMetaHub(input: StartLocalMetaHubInput): Promise<LocalMetaHubStatus>
  stopLocalMetaHub(): Promise<LocalMetaHubStatus>
}

export function bindMetaHubClient(client: ChannelClient): MetaHubClientOperations {
  const call = (command: string, input?: unknown): Promise<unknown> => (
    client.call('desktopBackend', command, input)
  )
  return {
    getMetaHubSettings: async () => MetaHubSettingsSchema.parse(
      await call('getMetaHubSettings'),
    ),
    updateMetaHubSettings: async (input) => MetaHubSettingsSchema.parse(
      await call('updateMetaHubSettings', input),
    ),
    getMetaHubStatus: async (target) => MetaHubRuntimeStateSchema.parse(
      await call('getMetaHubStatus', target),
    ),
    joinMetaHub: async (input) => MetaHubRuntimeStateSchema.parse(
      await call('joinMetaHub', input),
    ),
    disconnectMetaHub: async (target) => MetaHubRuntimeStateSchema.parse(
      await call('disconnectMetaHub', target),
    ),
    getLocalMetaHubStatus: async () => LocalMetaHubStatusSchema.parse(
      await call('getLocalMetaHubStatus'),
    ),
    startLocalMetaHub: async (input) => LocalMetaHubStatusSchema.parse(
      await call('startLocalMetaHub', input),
    ),
    stopLocalMetaHub: async () => LocalMetaHubStatusSchema.parse(
      await call('stopLocalMetaHub'),
    ),
  }
}
