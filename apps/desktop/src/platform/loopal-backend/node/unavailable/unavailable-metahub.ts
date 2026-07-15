import { CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import { type FakeMetaHubOperations } from '../fake/fake-metahub'

export function bindUnavailableMetaHub(reason: string): FakeMetaHubOperations {
  const fail = async (token = CancellationToken.None): Promise<never> => {
    throwIfCancelled(token)
    throw new Error(reason)
  }
  return {
    getMetaHubSettings: async (token = CancellationToken.None) => {
      throwIfCancelled(token)
      return {
        address: '', hubName: 'loopal-desktop', joinOnStart: false,
        startLocalOnLaunch: false, tokenConfigured: false,
      }
    },
    updateMetaHubSettings: async (_input, token) => fail(token),
    getMetaHubStatus: async (_target, token) => fail(token),
    joinMetaHub: async (_input, token) => fail(token),
    disconnectMetaHub: async (_target, token) => fail(token),
    getLocalMetaHubStatus: async (token = CancellationToken.None) => {
      throwIfCancelled(token); return { state: 'stopped' }
    },
    startLocalMetaHub: async (_input, token) => fail(token),
    stopLocalMetaHub: async (token) => fail(token),
  }
}
