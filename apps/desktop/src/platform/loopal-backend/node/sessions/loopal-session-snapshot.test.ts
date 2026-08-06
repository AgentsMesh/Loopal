import { describe, expect, it, vi } from 'vitest'
import { type SessionSummary } from '../../../../shared/contracts'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { loadMetaHubState } from '../federation/loopal-metahub-projection'
import { LoopalEventProjector } from '../projections/loopal-event-projector'
import { loadSessionDetail } from './loopal-session-snapshot'

const now = () => new Date('2026-07-11T12:00:00.000Z')
const session: SessionSummary = {
  id: 'session', workspaceId: 'workspace', title: 'Session', model: 'model', mode: 'agent',
  status: 'waiting', createdAt: now().toISOString(), updatedAt: now().toISOString(),
}

describe('loadSessionDetail', () => {
  it('normalizes missing message IDs and includes authoritative streaming text', async () => {
    const request = vi.fn(async (method: string, params?: unknown) => method === 'hub/list_agents'
      ? { agents: [
        { name: 'main', state: 'local' }, { name: 'worker', state: 'connected' },
      ] }
      : method === 'view/snapshot'
      ? (() => {
        const agent = (params as { agent?: string })?.agent ?? 'main'
        return {
        rev: 9,
        state: { agent: {
          name: agent, observable: { status: 'Running' },
          conversation: {
            streaming_text: agent === 'main' ? 'partial' : '',
            messages: agent === 'main' ? [{ role: 'tool', content: 'tool output' }] : [],
            ...(agent === 'main' ? {
              pending_permission: { id: 'permission', name: 'Read', input: 'README.md' },
            } : {
              pending_question: {
                id: 'question',
                questions: [{ question: 'Continue?', options: [], allow_multiple: false }],
                classifier_status: { kind: 'running', elapsed_ms: 10 },
              },
            }),
          },
        } },
        }
      })()
      : { agents: [
        { name: 'main', parent: null, children: ['worker'], lifecycle: 'running' },
        { name: 'worker', parent: 'main', children: [], lifecycle: 'running' },
      ] })
    const projector = new LoopalEventProjector(now, {
      append: vi.fn(), updateSession: vi.fn(), attention: vi.fn(),
    })
    const result = await loadSessionDetail(
      { request } as unknown as DesktopHostClient, session, now, projector,
    )
    expect(result).toMatchObject({
      revision: 9,
      detail: {
        conversation: [
          { id: expect.stringMatching(/^main-message-/), role: 'system', text: 'tool output' },
          { id: 'main-streaming-assistant', role: 'assistant', text: 'partial' },
        ],
        agents: [
          { id: 'main', status: 'running', controllable: true },
          { id: 'worker', status: 'running', controllable: true },
        ],
      },
      pendingAttention: [
        expect.objectContaining({
          kind: 'permission_requested', agentId: 'main',
          value: expect.objectContaining({ id: 'permission' }),
        }),
        expect.objectContaining({
          kind: 'question_requested', agentId: 'worker',
          value: expect.objectContaining({ id: 'question', classifier_running: true }),
        }),
      ],
    })
    expect(request).toHaveBeenCalledWith('view/snapshot', { agent: 'main' })
    expect(request).toHaveBeenCalledWith('hub/topology', {})
    expect(request).toHaveBeenCalledWith('view/snapshot', { agent: 'worker' })
    expect(request).not.toHaveBeenCalledWith('hub/status', expect.anything())
    expect(request).not.toHaveBeenCalledWith('meta/topology', expect.anything())
  })

  it('falls back to topology and retained agents when a child snapshot is unavailable', async () => {
    const main = {
      rev: 3,
      state: { agent: {
        name: 'main', observable: { status: 'WaitingForInput' },
        conversation: { messages: [] },
      } },
    }
    const request = vi.fn(async (method: string, params?: unknown) => {
      if (method === 'hub/topology') return { agents: [{
        name: 'worker', parent: 'main', children: [], lifecycle: 'finished', model: 'mini',
      }] }
      if (method === 'hub/list_agents') {
        return { agents: [{ name: 'main', state: 'connected' }, { name: 'worker', state: 'shadow' }] }
      }
      if ((params as { agent?: string })?.agent === 'worker') throw new Error('gone')
      return main
    })
    const previous = {
      session,
      conversation: [],
      artifacts: [{
        id: 'artifact', sessionId: session.id, title: 'result.md', kind: 'document' as const,
        uri: 'loopal-workspace://result.md', mediaType: 'text/markdown',
        producerAgentId: 'worker', createdAt: now().toISOString(),
      }],
      agents: [{
        id: 'worker', name: 'Worker', status: 'running' as const,
        conversation: [{
          id: 'result', role: 'assistant' as const, text: 'done', createdAt: now().toISOString(),
        }],
        telemetry: {
          turnCount: 1, inputTokens: 1, outputTokens: 1, cacheCreationTokens: 0,
          cacheReadTokens: 0, thinkingTokens: 0, contextWindow: 10,
          toolsInFlight: 0, toolCount: 0,
        },
      }, { id: 'retired', name: 'Retired', status: 'running' as const }],
    }
    const projector = new LoopalEventProjector(now, {
      append: vi.fn(), updateSession: vi.fn(), attention: vi.fn(),
    })
    const result = await loadSessionDetail(
      { request } as unknown as DesktopHostClient, session, now, projector, previous,
    )
    expect(result.detail.agents).toEqual(expect.arrayContaining([
      expect.objectContaining({ id: 'main', status: 'waiting', controllable: true }),
      expect.objectContaining({ id: 'worker', status: 'completed', controllable: false, conversation: previous.agents[0]!.conversation }),
      expect.objectContaining({ id: 'retired', status: 'completed', controllable: false }),
    ]))
    expect(result.detail.artifacts).toEqual(previous.artifacts)
  })

  it('rehydrates remote questions from qualified MetaHub agent snapshots', async () => {
    let topologyMode: 'present' | 'absent' | 'unavailable' = 'present'
    let remoteSnapshotAvailable = true
    let metaQueryFails = false
    const request = vi.fn(async (method: string, params?: unknown) => {
      if (method === 'hub/status') {
        if (metaQueryFails) throw new Error('MetaHub query failed')
        return {
          agent_count: 1,
          uplink: { connected: true, hub_name: 'hub-a', address: '127.0.0.1:9000' },
        }
      }
      if (method === 'meta/list_hubs') {
        return { hubs: [
          { name: 'hub-a', status: 'Connected', agent_count: 1, capabilities: [] },
          { name: 'hub-b', status: 'Connected', agent_count: 1, capabilities: [] },
        ] }
      }
      if (method === 'meta/topology') {
        return { hubs: [
          { hub: 'hub-a', topology: { agents: [{
            name: 'main', parent: null, children: [], lifecycle: 'running',
          }] } },
          { hub: 'hub-b', topology: topologyMode === 'unavailable'
            ? { error: 'hub-b topology unavailable' }
            : { agents: topologyMode === 'present' ? [{
              name: 'worker', parent: 'hub-a/main', children: [], lifecycle: 'running',
            }] : [] } },
        ] }
      }
      if (method === 'hub/topology') {
        return { agents: [{
          name: 'main', parent: null, children: [], lifecycle: 'running',
        }] }
      }
      if (method === 'hub/list_agents') {
        return { agents: [{ name: 'main', state: 'connected' }] }
      }
      if (method === 'view/snapshot') {
        const agent = (params as { agent?: string })?.agent ?? 'main'
        if (agent === 'hub-b/worker' && !remoteSnapshotAvailable) {
          throw new Error('no agent: hub-b/worker')
        }
        return {
          rev: agent === 'hub-b/worker' ? 17 : 3,
          state: { agent: {
            name: agent,
            observable: { status: agent === 'hub-b/worker' ? 'WaitingForInput' : 'Running' },
            conversation: agent === 'hub-b/worker' ? {
              messages: [],
              pending_question: {
                id: '1932b861-8f08-4906-8f24-b2685416b851',
                questions: [{
                  question: 'Continue remotely?', options: [], allow_multiple: false,
                }],
                classifier_status: { kind: 'running', elapsed_ms: 25 },
              },
            } : { messages: [] },
          } },
        }
      }
      throw new Error(`unexpected method: ${method}`)
    })
    const host = { request } as unknown as DesktopHostClient
    await loadMetaHubState(host, now(), true)
    request.mockClear()
    const projector = new LoopalEventProjector(now, {
      append: vi.fn(), updateSession: vi.fn(), attention: vi.fn(),
    })

    const result = await loadSessionDetail(host, session, now, projector)

    expect(request).toHaveBeenCalledWith('view/snapshot', { agent: 'hub-b/worker' })
    expect(request).not.toHaveBeenCalledWith('view/snapshot', { agent: 'hub-a/main' })
    expect(result.revisions).toMatchObject({ main: 3, 'hub-b/worker': 17 })
    expect(result.authoritativeRemoteAgents).toContain('hub-b/worker')
    expect(result.pendingAttention).toContainEqual({
      kind: 'question_requested',
      agentId: 'hub-b/worker',
      value: {
        id: '1932b861-8f08-4906-8f24-b2685416b851',
        questions: [{
          question: 'Continue remotely?', options: [], allow_multiple: false,
        }],
        classifier_running: true,
        classifier_status: { kind: 'running', elapsed_ms: 25 },
      },
    })
    expect(result.detail.agents).toContainEqual(expect.objectContaining({
      id: 'hub-b/worker', qualifiedName: 'hub-b/worker', controllable: true,
    }))

    remoteSnapshotAvailable = false
    topologyMode = 'unavailable'
    await loadMetaHubState(host, now(), true)
    request.mockClear()
    const unavailable = await loadSessionDetail(host, session, now, projector, result.detail)
    expect(request).toHaveBeenCalledWith('view/snapshot', { agent: 'hub-b/worker' })
    expect(unavailable.authoritativeRemoteAgents).not.toContain('hub-b/worker')

    metaQueryFails = true
    await loadMetaHubState(host, now(), true)
    request.mockClear()
    const knownRemote = new Set(['hub-b/worker'])
    const failed = await loadSessionDetail(
      host, session, now, projector, unavailable.detail, knownRemote,
    )
    expect(request).toHaveBeenCalledWith('view/snapshot', { agent: 'hub-b/worker' })
    expect(failed.authoritativeRemoteAgents).not.toContain('hub-b/worker')

    metaQueryFails = false
    topologyMode = 'absent'
    await loadMetaHubState(host, now(), true)
    request.mockClear()
    const absent = await loadSessionDetail(
      host, session, now, projector, failed.detail, knownRemote,
    )
    expect(request).toHaveBeenCalledWith('view/snapshot', { agent: 'hub-b/worker' })
    expect(absent.authoritativeRemoteAgents).toContain('hub-b/worker')
    expect(absent.pendingAttention).toEqual([])
  })

  it('keeps rendering while live-agent discovery is unavailable', async () => {
    const request = vi.fn(async (method: string) => {
      if (method === 'hub/list_agents') throw new Error('temporarily unavailable')
      if (method === 'hub/topology') return { agents: [] }
      return {
        rev: 1,
        state: { agent: {
          name: 'main', observable: { status: 'Running' }, conversation: { messages: [] },
        } },
      }
    })
    const projector = new LoopalEventProjector(now, {
      append: vi.fn(), updateSession: vi.fn(), attention: vi.fn(),
    })
    const result = await loadSessionDetail(
      { request } as unknown as DesktopHostClient, session, now, projector,
    )
    expect(result.detail.agents[0]).toMatchObject({
      id: 'main', status: 'running', controllable: false,
    })
  })
})
