import { describe, expect, it } from 'vitest'
import {
  LoopalDefaultSettingsSchema,
  LoopalSettingsValuesSchema,
  UpdateLoopalSettingsInputSchema,
} from './loopal-settings-contracts'

const settings = {
  model: 'claude-opus-4-8', modelRouting: {
    default: '', summarization: 'claude-haiku', classification: '', refine: '',
  }, permissionMode: 'ask_dangerous' as const,
  decisionMode: 'classifier' as const, sandboxPolicy: 'read_only' as const,
  thinking: { type: 'effort' as const, level: 'high' as const },
  maxContextTokens: 200_000, memoryEnabled: true, microcompactIdleMinutes: 30,
  telemetryEnabled: false, outputStyle: 'engineer',
}
const provider = { enabled: true, baseUrl: 'https://api.example.test/v1',
  apiKeyEnv: 'SAFE_API_KEY', apiKeyConfigured: true }
const projection = {
  providers: { anthropic: provider, openai: { ...provider, enabled: false }, google: provider },
  openaiCompatible: [],
  resolvedEntries: [{ key: 'providers.anthropic.api_key', value: '********' }],
  settingSources: ['project local overrides'],
}

describe('Loopal default settings contracts', () => {
  it('accepts the public redacted settings projection', () => {
    expect(LoopalDefaultSettingsSchema.parse({
      workspaceId: 'workspace', settings, configuredProviders: ['anthropic'], ...projection,
    })).toEqual({ workspaceId: 'workspace', settings, configuredProviders: ['anthropic'],
      ...projection })
  })

  it('accepts the complete reasoning effort range', () => {
    for (const level of ['none', 'low', 'medium', 'high', 'xhigh', 'max']) {
      expect(LoopalSettingsValuesSchema.safeParse({
        ...settings, thinking: { type: 'effort', level },
      }).success).toBe(true)
    }
  })

  it('strictly rejects secrets, unknown modes, and invalid numeric bounds', () => {
    expect(UpdateLoopalSettingsInputSchema.safeParse({
      workspaceId: 'workspace', settings: { ...settings, apiKey: 'secret' },
    }).success).toBe(false)
    expect(LoopalSettingsValuesSchema.safeParse({
      ...settings, permissionMode: 'unrestricted',
    }).success).toBe(false)
    expect(LoopalSettingsValuesSchema.safeParse({
      ...settings, microcompactIdleMinutes: 1441,
    }).success).toBe(false)
    expect(LoopalSettingsValuesSchema.safeParse({
      ...settings, thinking: { type: 'budget', tokens: 0 },
    }).success).toBe(false)
    expect(UpdateLoopalSettingsInputSchema.safeParse({
      workspaceId: 'workspace', settings,
      providerUpdates: { anthropic: { apiKey: 'write-only-secret' } },
    }).success).toBe(true)
    expect(UpdateLoopalSettingsInputSchema.safeParse({
      workspaceId: 'workspace', settings,
      providerUpdates: { openaiCompatible: [{
        name: 'local', baseUrl: 'https://local.example.test/v1',
        modelPrefix: 'local/', apiKey: 'write-only-secret',
      }] },
    }).success).toBe(true)
    expect(UpdateLoopalSettingsInputSchema.safeParse({
      workspaceId: 'workspace', settings,
      providerUpdates: { anthropic: { apiKey: 'secret', clearApiKey: true } },
    }).success).toBe(false)
    expect(UpdateLoopalSettingsInputSchema.safeParse({
      workspaceId: 'workspace', settings,
      providerUpdates: { openaiCompatible: [{
        name: 'local', apiKey: 'secret', clearApiKey: true,
      }] },
    }).success).toBe(false)
    expect(UpdateLoopalSettingsInputSchema.safeParse({
      workspaceId: 'workspace', settings,
      providerUpdates: { openai: { baseUrl: 'https://user:pass@example.test/?key=secret' } },
    }).success).toBe(false)
    expect(LoopalDefaultSettingsSchema.safeParse({
      workspaceId: 'workspace', settings, configuredProviders: [], ...projection,
      providers: { ...projection.providers, anthropic: { ...provider, apiKey: 'secret' } },
    }).success).toBe(false)
  })
})
