import { CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  type LocalMetaHubStatus,
  type StartLocalMetaHubInput,
} from '../../../../shared/contracts'
import { type LoopalMetaHubCoordinator } from './loopal-metahub-coordinator'
import { type LoopalMetaHubSettings } from './loopal-metahub-settings'

export interface LocalMetaHubOperations {
  getLocalMetaHubStatus(token?: CancellationToken): Promise<LocalMetaHubStatus>
  startLocalMetaHub(
    input: StartLocalMetaHubInput, token?: CancellationToken,
  ): Promise<LocalMetaHubStatus>
  stopLocalMetaHub(token?: CancellationToken): Promise<LocalMetaHubStatus>
}

export function bindLocalMetaHub(
  coordinator: LoopalMetaHubCoordinator,
  settings: LoopalMetaHubSettings,
): LocalMetaHubOperations {
  return {
    getLocalMetaHubStatus: async (token = CancellationToken.None) => {
      throwIfCancelled(token)
      return coordinator.status
    },
    startLocalMetaHub: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      const managed = await coordinator.start(input.bindAddress)
      throwIfCancelled(token)
      await settings.useManaged(managed.address, managed.token)
      return coordinator.status
    },
    stopLocalMetaHub: async (token = CancellationToken.None) => {
      throwIfCancelled(token)
      const status = coordinator.status
      const address = coordinator.ownedAddress
        ?? (status.state === 'running' ? status.address : undefined)
      await coordinator.stop()
      if (address) await settings.clearManaged(address)
      throwIfCancelled(token)
      return coordinator.status
    },
  }
}

export async function startLocalOnLaunch(
  coordinator: LoopalMetaHubCoordinator,
  settings: LoopalMetaHubSettings,
): Promise<void> {
  if (!settings.publicValue.startLocalOnLaunch) return
  try {
    const managed = await coordinator.start('127.0.0.1:0')
    await settings.useManaged(managed.address, managed.token)
  } catch {}
}
