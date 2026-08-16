import { projectMessages } from './loopal-message-projection'
import { richSnapshot } from './loopal-rich-projection.test-fixtures'
import { projectTool } from './loopal-tool-projection'
import {
  projectAgent, projectSessionView, retiredAgent, topologyAgent,
} from './loopal-view-projection'

const now = new Date('2026-07-11T12:00:00.000Z')

describe('Loopal rich projection edges', () => {
  it('handles duplicate fallback messages and cyclic inbox sources', () => {
    const snapshot = richSnapshot()
    const source: Record<string, unknown> = {}
    source.self = source
    const original = snapshot.state.agent.conversation.messages[3]!
    const { message_id: _id, ...withoutId } = original
    const duplicate = { ...structuredClone(withoutId), role: 'user', content: 'same' }
    const thinking = {
      ...snapshot.state.agent.conversation.messages[0]!, content: 'plain thinking',
    }
    const circular = {
      ...original, message_id: 'circular', tool_calls: [],
      inbox: { message_id: 'inbox', source, summary: null },
    }
    snapshot.state.agent.conversation.messages = [
      duplicate, structuredClone(duplicate), thinking, circular,
    ]
    snapshot.state.agent.conversation.streaming_text = ''
    snapshot.state.agent.conversation.streaming_thinking = ''
    const messages = projectMessages(snapshot, now)
    expect(messages[0]?.id).toMatch(/^worker-message-/)
    expect(messages[1]?.id).toBe(`${messages[0]?.id}-1`)
    expect(messages[0]).toMatchObject({ role: 'user', toolCalls: expect.any(Array) })
    expect(messages[2]).toMatchObject({
      role: 'thinking', text: 'plain thinking', thinkingTokens: 0,
    })
    expect(messages[3]?.inbox).toEqual({ source: 'agent' })
    expect(messages.some((message) => message.streaming)).toBe(false)
  })

  it('labels every structured inbox source without leaking wire JSON', () => {
    const snapshot = richSnapshot()
    const template = snapshot.state.agent.conversation.messages[3]!
    const sources = [
      { Agent: { hub: ['hub-b'], agent: 'main' } },
      { AgentResult: { child: { hub: ['hub-c'], agent: 'worker' } } },
      { Channel: { channel: 'ops', from: { hub: ['hub-d'], agent: 'relay' } } },
      { System: 'goal_continuation' },
    ]
    snapshot.state.agent.conversation.messages = sources.map((source, index) => ({
      ...template, message_id: `source-${index}`, tool_calls: [],
      inbox: { message_id: `inbox-${index}`, source, summary: null },
    }))
    snapshot.state.agent.conversation.streaming_text = ''
    snapshot.state.agent.conversation.streaming_thinking = ''
    expect(projectMessages(snapshot, now).map((message) => message.inbox?.source)).toEqual([
      'hub-b/main', 'hub-c/worker', 'hub-d/relay', 'system:goal_continuation',
    ])
  })

  it('retains bounded event notices across authoritative snapshot replacement', () => {
    const snapshot = richSnapshot()
    snapshot.state.agent.name = 'main'
    snapshot.state.agent.conversation.streaming_text = ''
    snapshot.state.agent.conversation.streaming_thinking = ''
    const previous = Array.from({ length: 70 }, (_, index) => ({
      id: `event-${index}`, role: 'system' as const, text: `notice ${index}`,
      agentId: index === 0 ? 'worker' : 'main', eventNotice: true,
      createdAt: new Date(now.getTime() + index * 1_000).toISOString(),
    }))
    const messages = projectMessages(snapshot, new Date(now.getTime() + 100_000), previous)
    const notices = messages.filter((message) => message.eventNotice)
    expect(notices).toHaveLength(64)
    expect(notices[0]?.text).toBe('notice 6')
    expect(notices.at(-1)?.text).toBe('notice 69')
    expect(notices.some((message) => message.agentId === 'worker')).toBe(false)
  })

  it('projects absent, malformed, and metadata-only tool fields', () => {
    const base = richSnapshot().state.agent.conversation.messages[3]!.tool_calls[0]!
    const { input: _input, ...withoutInput } = base
    expect(projectTool({
      ...withoutInput, summary: '', metadata: null,
      state: { state: 'running', last_progress: { tail: 3 } },
    })).toEqual(expect.objectContaining({ summary: 'Read', status: 'running' }))
    expect(projectTool({
      ...base, state: {
        state: 'done', duration: { secs: -1 }, outcome: { type: 'success', content: 4 },
      },
    })).toMatchObject({ status: 'succeeded', durationMs: 0 })
    expect(projectTool({
      ...base, metadata: { kind: 'failure', reason: 'reason' },
      state: { state: 'done', duration: {}, outcome: { type: 'failure', error: 4 } },
    })).toMatchObject({ status: 'failed', detail: 'failure: reason', durationMs: 0 })
    expect(projectTool({
      ...base, metadata: { kind: 'cancelled', cause: 'parent' }, state: { state: 'unknown' },
    })).toMatchObject({ detail: 'cancelled: parent' })
    expect(projectTool({
      ...base, metadata: { kind: 4 }, state: { state: 'unknown', duration: [] },
    }).detail).toBeUndefined()
    expect(projectTool({
      ...base, metadata: [], state: { state: 'unknown', duration: 'invalid' },
    }).durationMs).toBeUndefined()
  })

  it('projects empty resources and every aggregate status fallback', () => {
    const snapshot = richSnapshot()
    const state = snapshot.state
    const task = state.tasks[0]!
    state.tasks = [
      { ...task, status: 'completed', active_form: null },
      { ...task, id: 'pending', status: 'unknown', active_form: null },
    ]
    const background = state.bg_tasks.bg!
    state.bg_tasks = {
      complete: { ...background, id: 'complete', status: 'Completed' },
      killed: { ...background, id: 'killed', status: 'Killed' },
      running: { ...background, id: 'running', status: 'Unknown' },
    }
    state.crons = [{ ...state.crons[0]!, next_fire_unix_ms: null }]
    state.mcp_status = null
    state.thread_goal = null
    delete state.hub_degraded_since_ms
    const view = projectSessionView(snapshot)
    expect(view.tasks.map((item) => item.status)).toEqual(['completed', 'pending'])
    expect(view.backgroundTasks.map((item) => item.status)).toEqual([
      'completed', 'killed', 'running',
    ])
    expect(view.crons[0]?.nextFireAt).toBeUndefined()
    expect(view.mcpServers).toEqual([])
    expect(view.goal).toBeUndefined()
    expect(view.hubDegradedSince).toBeUndefined()

    const goal = richSnapshot()
    for (const status of ['paused', 'complete', 'infeasible', 'unknown']) {
      goal.state.thread_goal!.status = status
      expect(projectSessionView(goal).goal?.status).toBe(
        status === 'unknown' ? 'active' : status,
      )
    }
  })

  it('covers topology and agent fallbacks without inventing state', () => {
    const snapshot = richSnapshot()
    snapshot.state.agent.parent = 'remote/'
    snapshot.state.agent.observable.model = ''
    for (const tool of snapshot.state.agent.conversation.messages[3]!.tool_calls) {
      if (tool.state.state === 'pending' || tool.state.state === 'running') tool.summary = ''
    }
    const previous = { id: 'worker', name: 'Old', status: 'waiting' as const, conversation: [] }
    expect(projectAgent(snapshot, undefined, now, previous)).toMatchObject({
      parentId: 'remote/', children: [], status: 'running',
    })
    expect(topologyAgent({
      name: 'unknown', parent: 'remote/', children: [], lifecycle: 'unknown' as never,
    }, { ...previous, telemetry: {
      turnCount: 0, inputTokens: 0, outputTokens: 0, cacheCreationTokens: 0,
      cacheReadTokens: 0, thinkingTokens: 0, contextWindow: 0, toolsInFlight: 0, toolCount: 0,
    }, view: projectSessionView(snapshot) })).toMatchObject({
      status: 'idle', parentId: 'remote/', view: { revision: 12 },
    })
    const main = { id: 'main', name: 'Loopal', status: 'running' as const }
    expect(retiredAgent(main)).toBe(main)
  })
})
