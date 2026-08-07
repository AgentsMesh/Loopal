import { describe, expect, it, vi } from 'vitest'
import { Emitter, type Event } from '../../../../base/common/event'
import {
  type RuntimeSummary,
  type SessionDetail,
  type SessionSummary,
} from '../../../../shared/contracts'
import { type ChannelClient } from '../../../ipc/common/channel'
import { DesktopBackendClient } from './backend-client'

const now = '2026-07-11T12:00:00.000Z'
const session = {
  id: 'session', workspaceId: 'workspace', title: 'Session', model: 'gpt-5', mode: 'agent',
  status: 'running', createdAt: now, updatedAt: now, activeRuntimeId: 'runtime',
} satisfies SessionSummary
const runtime = {
  id: 'runtime', sessionId: 'session', workspaceId: 'workspace', generation: 1,
  state: 'ready', rootAgent: 'main', startedAt: now,
} satisfies RuntimeSummary
const detail = {
  session, conversation: [], agents: [], artifacts: [],
} satisfies SessionDetail
const createInput = { authorizationId: '5d0c638c-d44c-4f47-818b-62e6b599e31c', launchMode: 'directory' } as const
type ChannelRequest = (
  channel: string,
  command: string,
  input?: unknown,
) => Promise<unknown>

class TestChannel implements ChannelClient {
  readonly request = vi.fn<ChannelRequest>()

  constructor(
    private readonly events: Emitter<unknown>,
    handler: ChannelRequest,
  ) {
    this.request.mockImplementation(handler)
  }

  async call<T>(channel: string, command: string, input?: unknown): Promise<T> {
    return await this.request(channel, command, input) as T
  }

  listen<T>(): Event<T> {
    return (listener) => this.events.event((value) => listener(value as T))
  }

  dispose(): void {
    this.events.dispose()
  }
}

describe('DesktopBackendClient', () => {
  it('calls the explicit session façade and validates results', async () => {
    const event = new Emitter<unknown>()
    const client = new TestChannel(event, async (_channel, command) => {
      if (command === 'bootstrap') return {
        protocolVersion: 2, hostStatus: 'ready', workspaces: [],
        sessions: [session], runtimes: [runtime], activeSessionId: session.id,
      }
      if (command === 'openSession' || command === 'createSession') return detail
      if (command === 'restartSession') return runtime
      if (command === 'getDesktopPreferences') return { locale: 'system' }
      if (command === 'updateDesktopPreferences') return { locale: 'zh-CN' }
      if (command === 'getLoopalSettings' || command === 'updateLoopalSettings') return {
        workspaceId: 'workspace',
        settings: {
          model: 'gpt-5', modelRouting: {
            default: '', summarization: '', classification: '', refine: '',
          },
          permissionMode: 'bypass', decisionMode: 'manual',
          sandboxPolicy: 'default_write', thinking: { type: 'auto' },
          maxContextTokens: 0, memoryEnabled: true, microcompactIdleMinutes: 60,
          telemetryEnabled: true, outputStyle: '',
        },
        configuredProviders: [],
        providers: {
          anthropic: emptyProvider(), openai: emptyProvider(), google: emptyProvider(),
        },
        openaiCompatible: [],
        resolvedEntries: [{ key: 'model', value: 'gpt-5' }],
        settingSources: ['defaults'],
      }
      return command === 'controlAgent' ? { status: 'queued' } : undefined
    })
    const backend = new DesktopBackendClient(client)
    await expect(backend.bootstrap()).resolves.toMatchObject({ protocolVersion: 2 })
    await expect(backend.openSession('session')).resolves.toMatchObject({ session })
    await expect(backend.createSession(createInput))
      .resolves.toMatchObject({ session })
    await backend.stopSession('session')
    await expect(backend.restartSession('session')).resolves.toMatchObject(runtime)
    await backend.sendMessage('session', 'hello')
    expect(client.request).toHaveBeenCalledWith('desktopBackend', 'sendMessage', {
      sessionId: 'session', text: 'hello',
    })
    await backend.sendMessage('session', 'child', 'worker')
    expect(client.request).toHaveBeenCalledWith('desktopBackend', 'sendMessage', {
      sessionId: 'session', text: 'child', agentId: 'worker',
    })
    const image = {
      name: 'pixel.png', mediaType: 'image/png' as const, data: 'iVBORw==', sizeBytes: 4,
    }
    await backend.sendMessage('session', '', 'worker', [image])
    expect(client.request).toHaveBeenCalledWith('desktopBackend', 'sendMessage', {
      sessionId: 'session', text: '', agentId: 'worker', images: [image],
    })
    const target = {
      sessionId: 'session', runtimeId: 'runtime', generation: 1, agentId: 'main',
    }
    await backend.interruptAgent(target)
    await backend.controlAgent({ target, command: { type: 'mode', mode: 'plan' } })
    expect(client.request).toHaveBeenCalledWith('desktopBackend', 'controlAgent', {
      target, command: { type: 'mode', mode: 'plan' },
    })
    await expect(backend.getDesktopPreferences()).resolves.toEqual({ locale: 'system' })
    await expect(backend.updateDesktopPreferences({ locale: 'zh-CN' })).resolves
      .toEqual({ locale: 'zh-CN' })
    const defaults = await backend.getLoopalSettings('workspace')
    await backend.updateLoopalSettings({
      workspaceId: 'workspace', settings: defaults.settings,
    })
    expect(client.request).toHaveBeenCalledWith(
      'desktopBackend', 'getLoopalSettings', { workspaceId: 'workspace' },
    )
    expect(client.request).toHaveBeenCalledWith(
      'desktopBackend', 'updateLoopalSettings', {
        workspaceId: 'workspace', settings: defaults.settings,
      },
    )
    const listener = vi.fn()
    const unsubscribe = backend.onEvent(listener)
    event.fire({ type: 'host_status', status: 'ready' })
    expect(listener).toHaveBeenCalledWith({ type: 'host_status', status: 'ready' })
    unsubscribe()
  })

  it('rejects malformed backend results and events', async () => {
    const onListenerError = vi.fn()
    const event = new Emitter<unknown>({ onListenerError })
    const client = new TestChannel(event, async () => ({ invalid: true }))
    const backend = new DesktopBackendClient(client)
    await expect(backend.bootstrap()).rejects.toThrow()
    await expect(backend.openSession('session')).rejects.toThrow()
    await expect(backend.createSession(createInput)).rejects.toThrow()
    await expect(backend.restartSession('session')).rejects.toThrow()
    await expect(backend.getMetaHubSettings()).rejects.toThrow()
    await expect(backend.getLoopalSettings('workspace')).rejects.toThrow()
    await expect(backend.getDesktopPreferences()).rejects.toThrow()
    await expect(backend.getMetaHubStatus({
      sessionId: 'session', runtimeId: 'runtime', generation: 1,
    })).rejects.toThrow()
    await expect(backend.getLocalMetaHubStatus()).rejects.toThrow()
    const listener = vi.fn()
    const unsubscribe = backend.onEvent(listener)
    expect(() => event.fire({ invalid: true })).not.toThrow()
    expect(listener).not.toHaveBeenCalled()
    expect(onListenerError).toHaveBeenCalledOnce()
    unsubscribe()
  })

  it('validates and forwards every MetaHub operation', async () => {
    const event = new Emitter<unknown>()
    const state = {
      state: 'connected', hubs: [], topology: [], refreshedAt: now,
    }
    const client = new TestChannel(event, async (_channel, command) => {
      if (command === 'getMetaHubSettings' || command === 'updateMetaHubSettings') return {
        address: 'meta:9', hubName: 'desktop-a', joinOnStart: true,
        startLocalOnLaunch: false, tokenConfigured: true,
      }
      if (command === 'getLocalMetaHubStatus' || command === 'startLocalMetaHub') {
        return { state: 'running', address: '127.0.0.1:9' }
      }
      if (command === 'stopLocalMetaHub') return { state: 'stopped' }
      return state
    })
    const backend = new DesktopBackendClient(client)
    const target = { sessionId: 'session', runtimeId: 'runtime', generation: 1 }
    await expect(backend.getMetaHubSettings()).resolves.toMatchObject({ tokenConfigured: true })
    await expect(backend.updateMetaHubSettings({
      address: 'meta:9', hubName: 'desktop-a', joinOnStart: true,
      startLocalOnLaunch: false, token: 'secret',
    })).resolves.toMatchObject({ address: 'meta:9' })
    await expect(backend.getMetaHubStatus(target)).resolves.toMatchObject({ state: 'connected' })
    await expect(backend.joinMetaHub({ ...target, token: 'secret' })).resolves
      .toMatchObject({ state: 'connected' })
    await expect(backend.disconnectMetaHub(target)).resolves.toMatchObject({ state: 'connected' })
    await expect(backend.getLocalMetaHubStatus()).resolves.toMatchObject({ state: 'running' })
    await expect(backend.startLocalMetaHub({ bindAddress: '127.0.0.1:0' })).resolves
      .toMatchObject({ state: 'running' })
    await expect(backend.stopLocalMetaHub()).resolves.toEqual({ state: 'stopped' })
    expect(client.request).toHaveBeenCalledWith('desktopBackend', 'joinMetaHub', {
      ...target, token: 'secret',
    })
  })
})
function emptyProvider() {
  return { enabled: false, baseUrl: '', apiKeyEnv: '', apiKeyConfigured: false }
}
