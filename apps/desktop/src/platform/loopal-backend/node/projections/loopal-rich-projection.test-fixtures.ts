import { ViewSnapshotSchema, type ViewSnapshot } from '../runtime/loopal-wire'

const now = new Date('2026-07-11T12:00:00.000Z')

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
      workflows: { active: [{
        id: 'wrun_ship', run_goal: 'Ship it', state: 'running', revision: 3,
        output_node: 'publish', created_at_unix_ms: 1_700_000_000_000,
        updated_at_unix_ms: 1_700_000_100_000,
        counts: {
          pending: 1, ready: 0, active: 1, succeeded: 2,
          failed: 0, cancelled: 0, skipped: 0,
        },
      }], recent: [] },
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
