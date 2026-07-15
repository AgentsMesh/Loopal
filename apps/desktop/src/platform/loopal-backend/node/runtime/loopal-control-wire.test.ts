import { describe, expect, it } from 'vitest'
import { type AgentControlCommand } from '../../../../shared/contracts'
import { toHubControlCommand } from './loopal-control-wire'

describe('Loopal control wire mapping', () => {
  it('maps the complete Desktop allowlist to Rust serde shapes', () => {
    const cases: readonly (readonly [AgentControlCommand, unknown])[] = [
      [{ type: 'mode', mode: 'act' }, { ModeSwitch: 'Act' }],
      [{ type: 'mode', mode: 'plan' }, { ModeSwitch: 'Plan' }],
      [{ type: 'clear' }, 'Clear'],
      [{ type: 'compact' }, { Compact: { instructions: null } }],
      [{ type: 'compact', instructions: 'keep tests' }, {
        Compact: { instructions: 'keep tests' },
      }],
      [{ type: 'model', model: 'gpt-5' }, { ModelSwitch: 'gpt-5' }],
      [{ type: 'rewind', turnIndex: 3 }, { Rewind: { turn_index: 3 } }],
      [{ type: 'thinking', config: { type: 'effort', level: 'high' } }, {
        ThinkingSwitch: '{"type":"effort","level":"high"}',
      }],
      [{ type: 'permission', mode: 'ask_any_write' }, {
        PermissionModeSwitch: 'ask_any_write',
      }],
      [{ type: 'decision', mode: 'manual' }, { DecisionModeSwitch: 'manual' }],
      [{ type: 'sandbox', policy: 'default_write' }, {
        SandboxPolicySwitch: 'default_write',
      }],
      [{ type: 'suspend' }, 'Suspend'], [{ type: 'unsuspend' }, 'Unsuspend'],
      [{ type: 'mcp_status' }, 'QueryMcpStatus'],
      [{ type: 'mcp_reconnect', server: 'fs' }, { McpReconnect: { server: 'fs' } }],
      [{ type: 'mcp_disconnect', server: 'fs' }, { McpDisconnect: { server: 'fs' } }],
      [{ type: 'background_task_kill', id: 'bg' }, { BgTaskKill: { id: 'bg' } }],
      [{ type: 'cron_delete', id: 'cron' }, { CronDelete: { id: 'cron' } }],
    ]
    for (const [command, expected] of cases) {
      expect(toHubControlCommand(command)).toEqual(expected)
    }
  })
})
