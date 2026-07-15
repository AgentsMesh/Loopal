import { CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  UpdateLoopalSettingsInputSchema,
  type LoopalBuiltInProviders,
  type LoopalDefaultSettings,
  type LoopalOpenAiCompatibleSettings,
  type LoopalProviderUpdates,
} from '../../../../shared/contracts'
import { type LoopalSettingsOperations } from '../settings/loopal-settings-service'
export type { LoopalSettingsOperations } from '../settings/loopal-settings-service'

export function bindFakeLoopalSettings(workspaceId: string): LoopalSettingsOperations {
  let value: LoopalDefaultSettings = {
    workspaceId,
    settings: {
      model: 'claude-opus-4-8', modelRouting: emptyRouting(),
      permissionMode: 'bypass', decisionMode: 'manual',
      sandboxPolicy: 'default_write', thinking: { type: 'auto' }, maxContextTokens: 0,
      memoryEnabled: true, microcompactIdleMinutes: 60,
      telemetryEnabled: true, outputStyle: '',
    },
    configuredProviders: ['test-provider'],
    providers: emptyProviders(),
    openaiCompatible: [],
    resolvedEntries: [{ key: 'model', value: 'claude-opus-4-8' }],
    settingSources: ['project local overrides'],
  }
  const requireWorkspace = (input: string): void => {
    if (input !== workspaceId) throw new Error(`Unknown workspace: ${input}`)
  }
  return {
    getLoopalSettings: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      requireWorkspace(input)
      return structuredClone(value)
    },
    updateLoopalSettings: async (input, token = CancellationToken.None) => {
      throwIfCancelled(token)
      const parsed = UpdateLoopalSettingsInputSchema.parse(input)
      requireWorkspace(parsed.workspaceId)
      const providers = applyProviders(value.providers, parsed.providerUpdates ?? {})
      const openaiCompatible = applyCompatible(
        value.openaiCompatible, parsed.providerUpdates?.openaiCompatible ?? [],
      )
      value = {
        ...value, settings: parsed.settings, providers, openaiCompatible,
        configuredProviders: [
          'test-provider',
          ...Object.entries(providers).filter(([, item]) => item.enabled).map(([name]) => name),
          ...openaiCompatible.map((item) => `openai-compatible: ${item.name}`),
        ],
        resolvedEntries: resolvedEntries(parsed.settings),
      }
      return structuredClone(value)
    },
  }
}

function applyCompatible(
  current: LoopalOpenAiCompatibleSettings[],
  updates: NonNullable<LoopalProviderUpdates['openaiCompatible']>,
): LoopalOpenAiCompatibleSettings[] {
  const next = structuredClone(current)
  for (const update of updates) {
    const index = next.findIndex((item) => item.name === update.name)
    if (update.remove) {
      if (index >= 0) next.splice(index, 1)
      continue
    }
    const item = index >= 0 ? next[index]! : {
      name: update.name, baseUrl: '', apiKeyEnv: '', modelPrefix: '', apiKeyConfigured: false,
    }
    const changed = {
      ...item,
      baseUrl: update.baseUrl ?? item.baseUrl,
      apiKeyEnv: update.apiKeyEnv ?? item.apiKeyEnv,
      modelPrefix: update.modelPrefix ?? item.modelPrefix,
      apiKeyConfigured: update.apiKey !== undefined
        ? true : update.clearApiKey ? false : item.apiKeyConfigured,
    }
    if (index >= 0) next[index] = changed
    else next.push(changed)
  }
  return next
}

function emptyRouting() {
  return { default: '', summarization: '', classification: '', refine: '' }
}

function emptyProviders(): LoopalBuiltInProviders {
  const empty = () => ({ enabled: false, baseUrl: '', apiKeyEnv: '', apiKeyConfigured: false })
  return { anthropic: empty(), openai: empty(), google: empty() }
}

function applyProviders(
  current: LoopalBuiltInProviders,
  updates: LoopalProviderUpdates,
): LoopalBuiltInProviders {
  const next = structuredClone(current)
  for (const name of ['anthropic', 'openai', 'google'] as const) {
    const update = updates[name]
    if (!update) continue
    if (update.remove || update.enabled === false) {
      next[name] = { enabled: false, baseUrl: '', apiKeyEnv: '', apiKeyConfigured: false }
      continue
    }
    const item = next[name]
    const changes = update.baseUrl !== undefined || update.apiKeyEnv !== undefined
      || update.apiKey !== undefined || update.clearApiKey
    next[name] = {
      enabled: update.enabled ?? (changes ? true : item.enabled),
      baseUrl: update.baseUrl ?? item.baseUrl,
      apiKeyEnv: update.apiKeyEnv ?? item.apiKeyEnv,
      apiKeyConfigured: update.apiKey !== undefined
        ? true : update.clearApiKey ? false : item.apiKeyConfigured,
    }
  }
  return next
}

function resolvedEntries(settings: LoopalDefaultSettings['settings']) {
  return [
    { key: 'model', value: settings.model },
    ...Object.entries(settings.modelRouting).map(([key, value]) => ({
      key: `model_routing.${key}`, value: value || '(default)',
    })),
  ]
}

export function bindUnavailableLoopalSettings(reason: string): LoopalSettingsOperations {
  const fail = (token: CancellationToken): never => {
    throwIfCancelled(token)
    throw new Error(reason)
  }
  return {
    getLoopalSettings: async (_workspaceId, token) => fail(token),
    updateLoopalSettings: async (_input, token) => fail(token),
  }
}
