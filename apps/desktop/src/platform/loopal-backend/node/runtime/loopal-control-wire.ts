import { type AgentControlCommand } from '../../../../shared/contracts'

export function toHubControlCommand(command: AgentControlCommand): unknown {
  switch (command.type) {
    case 'mode':
      return { ModeSwitch: command.mode === 'act' ? 'Act' : 'Plan' }
    case 'clear':
      return 'Clear'
    case 'compact':
      return { Compact: { instructions: command.instructions ?? null } }
    case 'model':
      return { ModelSwitch: command.model }
    case 'rewind':
      return { Rewind: { turn_index: command.turnIndex } }
    case 'thinking':
      return { ThinkingSwitch: JSON.stringify(command.config) }
    case 'permission':
      return { PermissionModeSwitch: command.mode }
    case 'decision':
      return { DecisionModeSwitch: command.mode }
    case 'sandbox':
      return { SandboxPolicySwitch: command.policy }
    case 'suspend':
      return 'Suspend'
    case 'unsuspend':
      return 'Unsuspend'
    case 'mcp_status':
      return 'QueryMcpStatus'
    case 'mcp_reconnect':
      return { McpReconnect: { server: command.server } }
    case 'mcp_disconnect':
      return { McpDisconnect: { server: command.server } }
    case 'background_task_kill':
      return { BgTaskKill: { id: command.id } }
    case 'cron_delete':
      return { CronDelete: { id: command.id } }
  }
}
