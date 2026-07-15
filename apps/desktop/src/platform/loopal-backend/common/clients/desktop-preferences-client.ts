import {
  DesktopPreferencesSchema,
  type DesktopPreferences,
  type UpdateDesktopPreferencesInput,
} from '../../../../shared/contracts'
import { type ChannelClient } from '../../../ipc/common/channel'

export interface DesktopPreferencesClientOperations {
  getDesktopPreferences(): Promise<DesktopPreferences>
  updateDesktopPreferences(input: UpdateDesktopPreferencesInput): Promise<DesktopPreferences>
}

export function bindDesktopPreferencesClient(
  client: ChannelClient,
): DesktopPreferencesClientOperations {
  return {
    getDesktopPreferences: async () => DesktopPreferencesSchema.parse(
      await client.call('desktopBackend', 'getDesktopPreferences'),
    ),
    updateDesktopPreferences: async (input) => DesktopPreferencesSchema.parse(
      await client.call('desktopBackend', 'updateDesktopPreferences', input),
    ),
  }
}
