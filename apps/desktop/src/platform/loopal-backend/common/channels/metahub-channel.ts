import { type CancellationToken } from '../../../../base/common/cancellation'
import {
  JoinMetaHubInputSchema,
  LocalMetaHubStatusSchema,
  MetaHubRuntimeStateSchema,
  MetaHubRuntimeTargetSchema,
  MetaHubSettingsSchema,
  StartLocalMetaHubInputSchema,
  UpdateMetaHubSettingsInputSchema,
} from '../../../../shared/contracts'
import { type DesktopBackend } from '../backend'

type MetaHubCall = { readonly handled: false } | { readonly handled: true; readonly value: unknown }

export async function callMetaHubBackend(
  backend: DesktopBackend,
  command: string,
  arg: unknown,
  token: CancellationToken,
): Promise<MetaHubCall> {
  switch (command) {
    case 'getMetaHubSettings':
      return handled(MetaHubSettingsSchema.parse(await backend.getMetaHubSettings(token)))
    case 'updateMetaHubSettings': {
      const input = UpdateMetaHubSettingsInputSchema.parse(arg)
      return handled(MetaHubSettingsSchema.parse(await backend.updateMetaHubSettings(input, token)))
    }
    case 'getMetaHubStatus': {
      const input = MetaHubRuntimeTargetSchema.parse(arg)
      return handled(MetaHubRuntimeStateSchema.parse(await backend.getMetaHubStatus(input, token)))
    }
    case 'joinMetaHub': {
      const input = JoinMetaHubInputSchema.parse(arg)
      return handled(MetaHubRuntimeStateSchema.parse(await backend.joinMetaHub(input, token)))
    }
    case 'disconnectMetaHub': {
      const input = MetaHubRuntimeTargetSchema.parse(arg)
      return handled(MetaHubRuntimeStateSchema.parse(await backend.disconnectMetaHub(input, token)))
    }
    case 'getLocalMetaHubStatus':
      return handled(LocalMetaHubStatusSchema.parse(await backend.getLocalMetaHubStatus(token)))
    case 'startLocalMetaHub': {
      const input = StartLocalMetaHubInputSchema.parse(arg)
      return handled(LocalMetaHubStatusSchema.parse(await backend.startLocalMetaHub(input, token)))
    }
    case 'stopLocalMetaHub':
      return handled(LocalMetaHubStatusSchema.parse(await backend.stopLocalMetaHub(token)))
    default:
      return { handled: false }
  }
}

function handled(value: unknown): MetaHubCall { return { handled: true, value } }
