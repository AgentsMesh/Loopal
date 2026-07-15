import { describe, expect, it } from 'vitest'
import { CancellationToken } from '../../../../base/common/cancellation'
import {
  bindFakeLoopalSettings,
  bindUnavailableLoopalSettings,
} from './fake-loopal-settings'

describe('fake Loopal settings boundaries', () => {
  it('keeps typed defaults per workspace and validates updates', async () => {
    const service = bindFakeLoopalSettings('workspace')
    const before = await service.getLoopalSettings('workspace', CancellationToken.None)
    expect(before.settings.model).toBe('claude-opus-4-8')
    const settings = { ...before.settings, model: 'updated' }
    await expect(service.updateLoopalSettings({ workspaceId: 'workspace', settings },
      CancellationToken.None)).resolves.toMatchObject({ settings: { model: 'updated' } })
    await expect(service.getLoopalSettings('workspace', CancellationToken.None)).resolves
      .toMatchObject({ settings: { model: 'updated' } })
    const provider = await service.updateLoopalSettings({
      workspaceId: 'workspace', settings,
      providerUpdates: { anthropic: {
        enabled: true, baseUrl: 'https://api.example.test/v1',
        apiKeyEnv: 'ANTHROPIC_KEY', apiKey: 'write-only-test-value',
      } },
    }, CancellationToken.None)
    expect(provider.providers.anthropic).toEqual({
      enabled: true, baseUrl: 'https://api.example.test/v1',
      apiKeyEnv: 'ANTHROPIC_KEY', apiKeyConfigured: true,
    })
    expect(JSON.stringify(provider)).not.toContain('write-only-test-value')
    const cleared = await service.updateLoopalSettings({
      workspaceId: 'workspace', settings,
      providerUpdates: { anthropic: { clearApiKey: true } },
    }, CancellationToken.None)
    expect(cleared.providers.anthropic.apiKeyConfigured).toBe(false)
    await expect(service.getLoopalSettings('other', CancellationToken.None)).rejects
      .toThrow('Unknown workspace')
    await expect(service.updateLoopalSettings({
      workspaceId: 'workspace', settings: { ...settings, microcompactIdleMinutes: 1441 },
    }, CancellationToken.None)).rejects.toThrow()
    await expect(service.getLoopalSettings('workspace', CancellationToken.Cancelled)).rejects
      .toThrow('cancelled')
  })

  it('fails closed when the bundled Host is unavailable', async () => {
    const service = bindUnavailableLoopalSettings('sidecar missing')
    await expect(service.getLoopalSettings('workspace', CancellationToken.None)).rejects
      .toThrow('sidecar missing')
    const fallback = bindFakeLoopalSettings('workspace')
    const current = await fallback.getLoopalSettings('workspace', CancellationToken.None)
    await expect(service.updateLoopalSettings({
      workspaceId: 'workspace', settings: current.settings,
    }, CancellationToken.None)).rejects.toThrow('sidecar missing')
  })

  it('models built-in and compatible provider override lifecycles', async () => {
    const service = bindFakeLoopalSettings('workspace')
    const initial = await service.getLoopalSettings('workspace', CancellationToken.None)
    const update = (providerUpdates: NonNullable<Parameters<
      typeof service.updateLoopalSettings
    >[0]['providerUpdates']>) => service.updateLoopalSettings({
      workspaceId: 'workspace', settings: initial.settings, providerUpdates,
    }, CancellationToken.None)

    const created = await update({
      anthropic: { enabled: true },
      openai: {
        baseUrl: 'https://openai.example.test/v1', apiKeyEnv: 'OPENAI_TEST_KEY',
        apiKey: 'private-openai-key',
      },
      google: { enabled: false },
      openaiCompatible: [{
        name: 'custom', baseUrl: 'https://custom.example.test/v1',
        apiKeyEnv: 'CUSTOM_KEY', modelPrefix: 'custom/', apiKey: 'private-custom-key',
      }],
    })
    expect(created.providers).toMatchObject({
      anthropic: { enabled: true },
      openai: { enabled: true, apiKeyConfigured: true },
      google: { enabled: false },
    })
    expect(created.openaiCompatible[0]).toEqual({
      name: 'custom', baseUrl: 'https://custom.example.test/v1',
      apiKeyEnv: 'CUSTOM_KEY', modelPrefix: 'custom/', apiKeyConfigured: true,
    })
    expect(JSON.stringify(created)).not.toContain('private-')

    const changed = await update({
      anthropic: { remove: true },
      openai: { clearApiKey: true },
      google: { enabled: true },
      openaiCompatible: [
        { name: 'custom', clearApiKey: true },
        { name: 'second', baseUrl: 'https://second.example.test/v1' },
        { name: 'missing', remove: true },
      ],
    })
    expect(changed.providers).toMatchObject({
      anthropic: { enabled: false },
      openai: { enabled: true, apiKeyConfigured: false },
      google: { enabled: true },
    })
    expect(changed.openaiCompatible).toEqual([
      expect.objectContaining({ name: 'custom', apiKeyConfigured: false }),
      expect.objectContaining({ name: 'second', apiKeyConfigured: false }),
    ])

    const removed = await update({
      openai: { enabled: false },
      openaiCompatible: [{ name: 'custom', remove: true }],
    })
    expect(removed.providers.openai.enabled).toBe(false)
    expect(removed.openaiCompatible.map((item) => item.name)).toEqual(['second'])
    expect(removed.configuredProviders).toContain('openai-compatible: second')
  })
})
