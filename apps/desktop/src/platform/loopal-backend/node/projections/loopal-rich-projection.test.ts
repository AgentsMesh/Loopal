import { projectMessages } from './loopal-message-projection'
import { projectTool } from './loopal-tool-projection'
import {
  normalizeAgentStatus,
  projectAgent,
  projectSessionView,
  retiredAgent,
  topologyAgent,
} from './loopal-view-projection'
import { richSnapshot } from './loopal-rich-projection.test-fixtures'

const now = new Date('2026-07-11T12:00:00.000Z')

describe('rich Loopal view projection', () => {
  it('projects stable rich messages, streaming state, and every tool terminal state', () => {
    const snapshot = richSnapshot()
    const messages = projectMessages(snapshot, now)
    expect(messages).toEqual(expect.arrayContaining([
      expect.objectContaining({
        role: 'thinking', text: 'check the repository', thinkingTokens: 120,
      }),
      expect.objectContaining({ role: 'error', text: 'provider failed' }),
      expect.objectContaining({ role: 'welcome' }),
      expect.objectContaining({
        id: 'tools', role: 'assistant', imageCount: 2,
        skill: { name: 'review', userArgs: '--deep' },
        inbox: { source: 'parent', summary: 'delegated' },
      }),
      expect.objectContaining({ id: 'worker-streaming-thinking', streaming: true }),
      expect.objectContaining({ id: 'worker-streaming-assistant', streaming: true }),
    ]))
    const tools = messages.find((message) => message.id === 'tools')!.toolCalls!
    expect(tools.map((tool) => tool.status)).toEqual([
      'pending', 'running', 'succeeded', 'failed', 'stale', 'cancelled',
    ])
    expect(tools[1]).toMatchObject({ progress: 'halfway' })
    expect(tools[2]).toMatchObject({ output: 'ok', durationMs: 1500, batchId: 'batch' })
    expect(tools[3]).toMatchObject({ detail: 'boom' })
    expect(tools[4]).toMatchObject({ detail: 'watchdog_timeout' })
    expect(tools[5]).toMatchObject({ detail: 'user_interrupt' })
    const projectedAgain = projectMessages(
      snapshot, new Date('2026-07-12T12:00:00.000Z'), messages,
    )
    expect(projectedAgain.map(({ id, createdAt }) => ({ id, createdAt })))
      .toEqual(messages.map(({ id, createdAt }) => ({ id, createdAt })))
  })

  it('projects agents, topology fallback, retired nodes, and aggregate resources', () => {
    const snapshot = richSnapshot()
    const topology = {
      name: 'worker', parent: 'remote/main', children: ['child'],
      lifecycle: 'running' as const, model: 'topology-model',
    }
    const agent = projectAgent(snapshot, topology, now)
    expect(agent).toMatchObject({
      id: 'worker', parentId: 'main', children: ['child'], status: 'running',
      model: 'model', lastTool: 'Running',
      telemetry: { turnCount: 3, toolsInFlight: 2, toolCount: 6, contextWindow: 2000 },
      view: { revision: 12, thinkingActive: true },
    })
    const view = projectSessionView(snapshot)
    expect(view).toMatchObject({
      revision: 12, historyTruncated: true, retryBanner: 'retrying',
      compactBanner: 'compacting', hubDegradedSince: '2023-11-14T22:13:20.000Z',
      goal: { id: 'goal', status: 'active' },
      tasks: [{ id: '1', status: 'in_progress', activeForm: 'Testing' }],
      backgroundTasks: [{ id: 'bg', status: 'failed', exitCode: 7 }],
      crons: [{ id: 'cron', nextFireAt: '2023-11-14T22:15:00.000Z' }],
      mcpServers: [{ name: 'git', toolCount: 2 }],
      workflows: { active: [], recent: [] },
    })
    expect(['Starting', 'Running', 'WaitingForInput', 'Suspended', 'Finished', 'Error', '?']
      .map(normalizeAgentStatus)).toEqual([
        'starting', 'running', 'waiting', 'suspended', 'completed', 'failed', 'idle',
      ])
    expect(['spawning', 'running', 'finished', 'failed'].map((lifecycle) => (
      topologyAgent({
        name: lifecycle, children: [],
        lifecycle: lifecycle as 'spawning' | 'running' | 'finished' | 'failed',
        ...(lifecycle === 'failed' ? { error: 'spawn failed' } : {}),
      }).status
    ))).toEqual(['starting', 'running', 'completed', 'failed'])
    expect(topologyAgent({ name: 'bad', children: [], lifecycle: 'failed', error: 'boom' }).error)
      .toBe('boom')
    expect(['spawning', 'finished', 'failed'].map((lifecycle) => projectAgent(
      snapshot,
      { ...topology, lifecycle: lifecycle as 'spawning' | 'finished' | 'failed' },
      now,
    ).status)).toEqual(['starting', 'completed', 'failed'])
    expect(projectAgent(snapshot, { ...topology, lifecycle: 'failed', error: 'boom' }, now))
      .toMatchObject({ status: 'failed', error: 'boom' })
    expect(retiredAgent({ id: 'old', name: 'Old', status: 'running', lastTool: 'Read' }))
      .toEqual({ id: 'old', name: 'Old', status: 'completed' })
    const rootSnapshot = richSnapshot()
    rootSnapshot.state.agent.name = 'main'
    expect(projectSessionView(rootSnapshot).workflows.active).toEqual([
      expect.objectContaining({
        id: 'wrun_ship', state: 'running', revision: 3, runGoal: 'Ship it',
      }),
    ])
    const failed = { id: 'failed', name: 'Failed', status: 'failed' as const }
    expect(retiredAgent(failed)).toBe(failed)
  })

  it('handles unknown tool states and typed metadata fallbacks', () => {
    const base = richSnapshot().state.agent.conversation.messages[3]!.tool_calls[0]!
    expect(projectTool({ ...base, state: { state: 'unknown' } })).toMatchObject({ status: 'failed' })
    expect(projectTool({
      ...base, metadata: { kind: 'bytes_written', count: 42 },
      state: { state: 'stale' },
    })).toMatchObject({ detail: '42 bytes written' })
    expect(projectTool({
      ...base, metadata: { kind: 'custom' }, state: { state: 'stale' },
    })).toMatchObject({ detail: 'custom' })
  })

  it('does not project completed thinking as a live thinking row', () => {
    const snapshot = richSnapshot()
    snapshot.state.agent.conversation.streaming_thinking = ''
    snapshot.state.agent.conversation.thinking_active = false
    const messages = projectMessages(snapshot, now)
    expect(messages.some((entry) => entry.id === 'worker-streaming-thinking')).toBe(false)
    expect(messages).toEqual(expect.arrayContaining([
      expect.objectContaining({ role: 'thinking', text: 'check the repository' }),
      expect.objectContaining({ id: 'worker-streaming-assistant', streaming: true }),
    ]))
  })
})
