import { describe, expect, it, vi } from 'vitest'
import { CancellationToken } from '../../../../base/common/cancellation'
import { Emitter } from '../../../../base/common/event'
import { createBackendStub } from '../../../../../test/support/backend/backend-stub'
import { DesktopBackendChannel } from './backend-channel'
import { type DesktopEvent } from '../../../../shared/contracts'

describe('DesktopBackendChannel', () => {
  it('dispatches only its explicit command allowlist', async () => {
    const service = createBackendStub()
    const channel = new DesktopBackendChannel(service)
    await expect(channel.call({}, 'bootstrap', undefined, CancellationToken.None)).resolves.toMatchObject({
      protocolVersion: 2,
    })
    await expect(
      channel.call({}, 'openSession', { sessionId: 'session' }, CancellationToken.None),
    ).resolves.toMatchObject({ session: { id: 'session' } })
    await expect(channel.call(
      {}, 'createSession', {
        authorizationId: '5d0c638c-d44c-4f47-818b-62e6b599e31c', launchMode: 'directory',
      }, CancellationToken.None,
    )).resolves.toMatchObject({ session: { id: 'session-new' } })
    await expect(channel.call(
      {}, 'stopSession', { sessionId: 'session' }, CancellationToken.None,
    )).resolves.toBeUndefined()
    await expect(channel.call(
      {}, 'restartSession', { sessionId: 'session' }, CancellationToken.None,
    )).resolves.toMatchObject({ sessionId: 'session' })
    await expect(
      channel.call({}, 'sendMessage', {
        sessionId: 'session', text: 'hello', agentId: 'worker', images: [{
          name: 'pixel.png', mediaType: 'image/png', data: 'iVBORw==', sizeBytes: 4,
        }],
      }, CancellationToken.None),
    ).resolves.toBeUndefined()
    const target = {
      sessionId: 'session', runtimeId: 'runtime', generation: 1, agentId: 'main',
    }
    await expect(channel.call(
      {}, 'interruptAgent', target, CancellationToken.None,
    )).resolves.toBeUndefined()
    await expect(channel.call(
      {}, 'controlAgent', { target, command: { type: 'clear' } }, CancellationToken.None,
    )).resolves.toBeUndefined()
    await expect(channel.call(
      {}, 'getDesktopPreferences', undefined, CancellationToken.None,
    )).resolves.toEqual({ locale: 'system' })
    await expect(channel.call(
      {}, 'updateDesktopPreferences', { locale: 'zh-CN' }, CancellationToken.None,
    )).resolves.toEqual({ locale: 'zh-CN' })
    await expect(channel.call(
      {}, 'getLoopalSettings', { workspaceId: 'workspace' }, CancellationToken.None,
    )).resolves.toMatchObject({ settings: { model: 'gpt-5' } })
    const defaults = await service.getLoopalSettings('workspace', CancellationToken.None)
    await expect(channel.call(
      {}, 'updateLoopalSettings', {
        workspaceId: defaults.workspaceId, settings: defaults.settings,
      }, CancellationToken.None,
    )).resolves.toMatchObject({ workspaceId: 'workspace' })
    expect(service.openSession).toHaveBeenCalledWith('session', CancellationToken.None)
    expect(service.sendMessage).toHaveBeenCalledWith(
      'session', 'hello', CancellationToken.None, 'worker', [{
        name: 'pixel.png', mediaType: 'image/png', data: 'iVBORw==', sizeBytes: 4,
      }],
    )
    expect(service.interruptAgent).toHaveBeenCalledWith(target, CancellationToken.None)
    await expect(channel.call({}, 'secret', {}, CancellationToken.None)).rejects.toMatchObject({
      code: 'COMMAND_NOT_FOUND',
    })
  })

  it('validates command arguments and event names', async () => {
    const service = createBackendStub()
    const channel = new DesktopBackendChannel(service)
    await expect(
      channel.call({}, 'openSession', { sessionId: '' }, CancellationToken.None),
    ).rejects.toThrow()
    await expect(channel.call(
      {}, 'createSession', { workspaceId: 'workspace' }, CancellationToken.None,
    )).rejects.toThrow()
    await expect(
      channel.call({}, 'sendMessage', { sessionId: 'session', text: ' ' }, CancellationToken.None),
    ).rejects.toThrow()
    await expect(channel.call({}, 'sendMessage', {
      sessionId: 'session', text: '', images: [{
        name: 'fake.png', mediaType: 'image/png', data: 'iVBORw==', sizeBytes: 5,
      }],
    }, CancellationToken.None)).rejects.toThrow()
    await expect(channel.call(
      {}, 'sendMessage', { sessionId: 'session', text: 'ok', agentId: '' }, CancellationToken.None,
    )).rejects.toThrow()
    await expect(channel.call(
      {}, 'controlAgent', {
        target: { sessionId: 'session', runtimeId: 'runtime', generation: 0, agentId: 'main' },
        command: { type: 'resume_session', sessionId: 'other' },
      }, CancellationToken.None,
    )).rejects.toThrow()
    await expect(channel.call(
      {}, 'getLoopalSettings', { workspaceId: '' }, CancellationToken.None,
    )).rejects.toThrow()
    await expect(channel.call(
      {}, 'updateLoopalSettings', { workspaceId: 'workspace', settings: {} },
      CancellationToken.None,
    )).rejects.toThrow()
    await expect(channel.call(
      {}, 'updateDesktopPreferences', { locale: 'fr' }, CancellationToken.None,
    )).rejects.toThrow()
    expect(() => channel.listen({}, 'missing')).toThrow('Unknown desktop backend event')
  })

  it('validates and dispatches every MetaHub command', async () => {
    const service = createBackendStub()
    const channel = new DesktopBackendChannel(service)
    const target = { sessionId: 'session', runtimeId: 'runtime', generation: 1 }
    const token = CancellationToken.None
    await expect(channel.call({}, 'getMetaHubSettings', undefined, token)).resolves
      .toMatchObject({ tokenConfigured: false })
    const settings = {
      address: 'meta:9', hubName: 'desktop-a', joinOnStart: true,
      startLocalOnLaunch: false, token: 'secret',
    }
    await expect(channel.call({}, 'updateMetaHubSettings', settings, token)).resolves
      .toMatchObject({ address: 'meta:9' })
    await expect(channel.call({}, 'getMetaHubStatus', target, token)).resolves
      .toMatchObject({ state: 'disconnected' })
    await expect(channel.call({}, 'joinMetaHub', { ...target, token: 'secret' }, token)).resolves
      .toMatchObject({ state: 'connected' })
    await expect(channel.call({}, 'disconnectMetaHub', target, token)).resolves
      .toMatchObject({ state: 'disconnected' })
    await expect(channel.call({}, 'getLocalMetaHubStatus', undefined, token)).resolves
      .toEqual({ state: 'stopped' })
    await expect(channel.call({}, 'startLocalMetaHub', {
      bindAddress: '127.0.0.1:0',
    }, token)).resolves.toMatchObject({ state: 'running' })
    await expect(channel.call({}, 'stopLocalMetaHub', undefined, token)).resolves
      .toEqual({ state: 'stopped' })
    await expect(channel.call({}, 'joinMetaHub', { ...target, token: '' }, token)).rejects
      .toThrow()
    await expect(channel.call({}, 'startLocalMetaHub', { bindAddress: '' }, token)).rejects
      .toThrow()
  })

  it('validates and forwards backend events until disposal', () => {
    const events = new Emitter<DesktopEvent>()
    const service = createBackendStub({ onEvent: events.event })
    const channel = new DesktopBackendChannel(service)
    const listener = vi.fn()
    const subscription = channel.listen({}, 'event')(listener)
    const event: DesktopEvent = { type: 'host_status', status: 'ready' }

    events.fire(event)
    expect(listener).toHaveBeenCalledWith(event)
    subscription.dispose()
    events.fire({ type: 'host_status', status: 'stopped' })
    expect(listener).toHaveBeenCalledOnce()
  })
})
