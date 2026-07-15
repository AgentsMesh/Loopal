import { describe, expect, it, vi } from 'vitest'
import { type DesktopEvent } from '../../../../shared/contracts'
import { FakeHost, agentEvent, timestamp } from '../backend/loopal-backend.test-fixtures'
import { LoopalSessionDirectory } from './loopal-session-directory'
import {
  SessionRuntimeRegistry,
  type SessionRuntimeHandle,
  type SessionRuntimeStatusEvent,
} from '../runtime/session-runtime-registry'

function row(id: string) {
  return {
    id, title: `Catalog ${id}`, model: 'model', mode: 'agent',
    createdAt: '2026-07-11T10:00:00.000Z', updatedAt: '2026-07-11T11:00:00.000Z',
  }
}

function harness(sessionId = 'session') {
  const catalog = [row(sessionId)]
  const hosts: FakeHost[] = []
  let runtimeIndex = 0
  const registry = new SessionRuntimeRegistry({
    maxLive: 4,
    createRuntimeId: () => `runtime-${++runtimeIndex}`,
    createHost: (input) => {
      const host = new FakeHost(input.resumeSessionId ?? sessionId, catalog)
      hosts.push(host)
      return host
    },
  })
  const events: DesktopEvent[] = []
  const services: unknown[] = []
  const directory = new LoopalSessionDirectory(
    registry, () => timestamp, 'project', {
      event: (event) => events.push(event),
      service: (event) => services.push(event),
    },
  )
  return { catalog, hosts, registry, directory, events, services }
}

describe('LoopalSessionDirectory', () => {
  it('retains the latest detail after its runtime stops', async () => {
    const { registry, directory, catalog } = harness()
    const runtime = await registry.resume({
      workspaceId: 'workspace', cwd: '/workspace', sessionId: 'session',
    })
    directory.mergeCatalog(catalog, 'workspace')
    await directory.attach(runtime)
    await registry.stop(runtime.runtimeId)

    expect(directory.liveSession('session')).toBeUndefined()
    const detail = directory.detail('session')!
    expect(detail).toMatchObject({
      session: { status: 'stopped' },
      conversation: [expect.objectContaining({ text: 'Answer from session' })],
    })
    expect(detail.session.activeRuntimeId).toBeUndefined()
  })

  it('buffers pre-attach events, bounds overflow, and replays a resync', async () => {
    const { registry, directory, hosts, catalog, events } = harness()
    const runtime = await registry.resume({
      workspaceId: 'workspace', cwd: '/workspace', sessionId: 'session',
    })
    for (let revision = 3; revision < 75; revision += 1) {
      hosts[0]!.notification('agent/event', agentEvent('Running', revision, revision))
    }
    hosts[0]!.notification('agent/event', agentEvent('Running', 100, 100))
    directory.mergeCatalog(catalog, 'workspace')
    await directory.attach(runtime)
    await vi.waitFor(() => expect(events).toContainEqual(expect.objectContaining({
      type: 'session_detail_replaced',
    })))
    expect(directory.liveSession('session')).toBeDefined()
    expect(directory.runtime('runtime-1')).toMatchObject({ state: 'ready' })
  })

  it('preserves live catalog fields, replaces a different runtime, and creates fallback sessions', async () => {
    const { registry, directory, hosts, catalog, events, services } = harness()
    const runtime = await registry.resume({
      workspaceId: 'workspace', cwd: '/workspace', sessionId: 'session',
    })
    directory.mergeCatalog(catalog, 'workspace')
    const original = await directory.attach(runtime)
    const dispose = vi.spyOn(original, 'dispose')
    directory.mergeCatalog([{ ...catalog[0]!, title: 'Renamed in catalog' }], 'workspace', true)
    expect(directory.session('session')).toMatchObject({
      title: 'Renamed in catalog', activeRuntimeId: 'runtime-1', status: 'waiting',
    })

    const replacementHost = new FakeHost('session', catalog)
    await replacementHost.start()
    const replacement: SessionRuntimeHandle = {
      workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime-new',
      generation: 5, host: replacementHost,
    }
    await directory.attach(replacement)
    expect(dispose).toHaveBeenCalledOnce()
    expect(directory.runtimeValues().filter((value) => value.sessionId === 'session'))
      .toEqual([expect.objectContaining({ id: 'runtime-new', generation: 5 })])

    const fallbackHost = new FakeHost('fresh-session', catalog)
    await fallbackHost.start()
    await directory.attach({
      workspaceId: 'workspace', sessionId: 'fresh-session', runtimeId: 'runtime-fresh',
      generation: 6, host: fallbackHost,
    })
    expect(directory.session('fresh-session')?.title).toBe('Loopal session · project')
    const count = events.length
    const stale: SessionRuntimeStatusEvent = {
      workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime-stale',
      generation: 4, status: 'ready',
    }
    const internals = directory as unknown as {
      acceptStatus(event: SessionRuntimeStatusEvent): void
      acceptNotification(event: typeof stale & { method: string; params: unknown }): void
    }
    internals.acceptStatus(stale)
    internals.acceptNotification({ ...stale, method: 'workspace/unknown', params: {} })
    expect(events).toHaveLength(count)
    expect(services).toHaveLength(1)
    directory.dispose()
    expect(directory.liveSession('session')).toBeUndefined()
    expect(hosts).toHaveLength(1)
  })
})
