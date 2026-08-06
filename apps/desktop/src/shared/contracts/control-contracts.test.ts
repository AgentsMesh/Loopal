import { describe, expect, it } from 'vitest'
import {
  AgentControlCommandSchema,
  AgentControlInputSchema,
  AgentControlTargetSchema,
  ThinkingConfigSchema,
} from './control-contracts'

const target = {
  sessionId: 'session', runtimeId: 'runtime', generation: 2, agentId: 'main',
}

describe('desktop agent control contracts', () => {
  it('accepts only an exact live-runtime target', () => {
    expect(AgentControlTargetSchema.parse(target)).toEqual(target)
    for (const invalid of [
      { ...target, sessionId: '' },
      { ...target, runtimeId: '' },
      { ...target, generation: 0 },
      { ...target, generation: 1.5 },
      { ...target, agentId: '' },
      { ...target, extra: true },
    ]) expect(AgentControlTargetSchema.safeParse(invalid).success).toBe(false)
  })

  it('validates every exposed command without exposing session hot-swap', () => {
    const commands = [
      { type: 'mode', mode: 'act' }, { type: 'mode', mode: 'plan' },
      { type: 'clear' }, { type: 'compact' }, { type: 'compact', instructions: 'keep tests' },
      { type: 'model', model: 'gpt-5' }, { type: 'rewind', turnIndex: 0 },
      { type: 'thinking', config: { type: 'auto' } },
      { type: 'permission', mode: 'ask_dangerous' },
      { type: 'decision', mode: 'classifier' },
      { type: 'sandbox', policy: 'read_only' },
      { type: 'suspend' }, { type: 'unsuspend' },
      { type: 'mcp_status' },
      { type: 'mcp_reconnect', server: 'filesystem' },
      { type: 'mcp_disconnect', server: 'filesystem' },
      { type: 'background_task_kill', id: 'bg-1' }, { type: 'cron_delete', id: 'cron-1' },
    ]
    for (const command of commands) {
      expect(AgentControlInputSchema.safeParse({ target, command }).success).toBe(true)
    }
    expect(AgentControlCommandSchema.safeParse({ type: 'resume_session', sessionId: 'other' }).success)
      .toBe(false)
  })

  it('bounds enums, payloads, thinking JSON, and extra fields', () => {
    for (const config of [
      { type: 'auto' }, { type: 'disabled' },
      { type: 'effort', level: 'none' }, { type: 'effort', level: 'xhigh' },
      { type: 'effort', level: 'max' },
      { type: 'budget', tokens: 4_294_967_295 },
    ]) expect(ThinkingConfigSchema.safeParse(config).success).toBe(true)
    for (const command of [
      { type: 'mode', mode: 'agent' },
      { type: 'rewind', turnIndex: -1 },
      { type: 'thinking', config: { type: 'effort', level: 'extreme' } },
      { type: 'permission', mode: 'always' },
      { type: 'decision', mode: 'auto' },
      { type: 'sandbox', policy: 'workspace_write' },
      { type: 'clear', surprise: true },
      { type: 'goal_create', objective: 'not a Desktop control' },
      { type: 'goal_pause' }, { type: 'goal_resume' }, { type: 'goal_complete' },
      { type: 'goal_reopen' }, { type: 'goal_clear' },
    ]) expect(AgentControlCommandSchema.safeParse(command).success).toBe(false)
  })
})
