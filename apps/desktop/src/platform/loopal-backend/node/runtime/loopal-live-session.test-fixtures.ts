import { vi } from 'vitest'
import { type DesktopEvent, type SessionSummary } from '../../../../shared/contracts'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { LoopalLiveSession } from './loopal-live-session'
import { type SessionRuntimeHandle } from './session-runtime-registry'

export const liveSessionNow = () => new Date('2026-07-11T12:00:00.000Z')
export const liveSessionSummary: SessionSummary = {
  id: 'session', workspaceId: 'workspace', title: 'Session', model: 'model', mode: 'agent',
  status: 'waiting', activeRuntimeId: 'runtime',
  createdAt: liveSessionNow().toISOString(), updatedAt: liveSessionNow().toISOString(),
}

export function liveSessionEvent(payload: unknown, revision: number, agent = 'main') {
  return {
    agent_name: { hub: [], agent }, event_id: revision, turn_id: 1,
    correlation_id: 2, rev: revision, payload,
  }
}

export function liveSessionHarness(requestImpl?: DesktopHostClient['request']) {
  const request = vi.fn<DesktopHostClient['request']>(requestImpl ?? (async (method) => {
    if (method === 'view/snapshot') return {
      rev: 2,
      state: { agent: {
        name: 'main', observable: { status: 'WaitingForInput' },
        conversation: { streaming_text: '', messages: [] },
      } },
    }
    if (method === 'hub/topology') return { agents: [{
      name: 'main', parent: null, children: [], lifecycle: 'running', model: 'model',
    }] }
    if (method === 'hub/list_agents') {
      return { agents: [{ name: 'main', state: 'connected' }] }
    }
    if (method === 'hub/status') return { agent_count: 1, uplink: null }
    return { ok: true }
  }))
  const runtime = {
    workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime', generation: 1,
    host: { request } as unknown as DesktopHostClient,
  } satisfies SessionRuntimeHandle
  const events: DesktopEvent[] = []
  const summaries: SessionSummary[] = []
  const state = new LoopalLiveSession(runtime, liveSessionSummary, liveSessionNow, {
    event: (value) => events.push(value), summary: (value) => summaries.push(value),
  })
  return { state, request, events, summaries }
}
