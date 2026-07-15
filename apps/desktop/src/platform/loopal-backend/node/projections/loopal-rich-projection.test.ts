import { projectMessages } from './loopal-message-projection'
import { projectTool } from './loopal-tool-projection'
import {
  normalizeAgentStatus,
  projectAgent,
  projectSessionView,
  retiredAgent,
  topologyAgent,
} from './loopal-view-projection'
import { ViewSnapshotSchema, type ViewSnapshot } from '../runtime/loopal-wire'

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

export function richSnapshot(): ViewSnapshot {
  const tools = [
    tool('pending', {}),
    tool('running', { last_progress: { tail: 'halfway' } }),
    tool('done', {
      duration: { secs: 1, nanos: 500_000_000 }, outcome: { type: 'success', content: 'ok' },
    }, 'batch'),
    tool('done', { duration: { secs: 0, nanos: 1 }, outcome: { type: 'failure', error: 'boom' } }),
    tool('stale', { reason: 'watchdog_timeout' }),
    tool('cancelled', { cause: 'user_interrupt' }),
  ]
  return ViewSnapshotSchema.parse({
    rev: 12,
    state: {
      agent: {
        name: 'worker', parent: 'main', children: [],
        observable: {
          status: 'Running', turn_count: 3, input_tokens: 100, output_tokens: 50,
          model: 'model', mode: 'act', thinking_config: 'high',
          permission_mode: 'ask', decision_mode: 'manual', sandbox_policy: 'read_only',
        },
        conversation: {
          history_truncated: true, streaming_text: 'answering',
          streaming_thinking: 'considering', thinking_active: true,
          retry_banner: 'retrying', compact_banner: 'compacting',
          turn_count: 3, input_tokens: 100, output_tokens: 50, context_window: 2000,
          cache_creation_tokens: 10, cache_read_tokens: 20, thinking_tokens: 120,
          messages: [
            { role: 'thinking', content: '120\ncheck the repository' },
            { role: 'error', content: 'provider failed' },
            { role: 'welcome', content: 'hello' },
            {
              message_id: 'tools', role: 'assistant', content: 'working', image_count: 2,
              skill_info: { name: 'review', user_args: '--deep' },
              inbox: {
                message_id: 'inbox', source: { Agent: { agent: 'parent' } }, summary: 'delegated',
              },
              tool_calls: tools,
            },
          ],
        },
      },
      tasks: [{
        id: '1', subject: 'Test', description: 'Run tests', active_form: 'Testing',
        status: 'in_progress', blocked_by: [], blocks: ['2'],
      }],
      bg_tasks: { bg: {
        id: 'bg', description: 'build', status: 'Failed', exit_code: 7,
        output: 'failed', created_at_unix_ms: 1_700_000_000_000,
      } },
      crons: [{
        id: 'cron', cron_expr: '* * * * *', prompt: 'check', recurring: true,
        durable: true, next_fire_unix_ms: 1_700_000_100_000,
      }],
      mcp_status: [{
        name: 'git', transport: 'stdio', source: 'project', status: 'connected',
        tool_count: 2, resource_count: 1, prompt_count: 0, errors: [],
      }],
      thread_goal: {
        goal_id: 'goal', objective: 'Ship', status: 'active',
        created_at: now.toISOString(), updated_at: now.toISOString(),
      },
      hub_degraded_since_ms: 1_700_000_000_000,
    },
  })
}

function tool(state: string, fields: Record<string, unknown>, batchId?: string) {
  return {
    id: `tool-${state}-${JSON.stringify(fields).length}`,
    name: 'Read', summary: state === 'running' ? 'Running' : state,
    input: { path: 'README.md' }, state: { state, ...fields },
    ...(batchId ? { batch_id: batchId } : {}),
  }
}
