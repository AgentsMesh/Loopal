import { type DesktopEvent } from '../../../../shared/contracts'
import {
  LoopalCodeWorkbench,
  type CodeWorkbenchOperations,
  type CodeWorkbenchRuntimeRouter,
} from '../workspace/loopal-code-workbench'
import { bindCodeWorkbench } from '../workspace/loopal-code-workbench-bind'
import { projectCodeWorkbenchEvent } from '../workspace/loopal-code-events'
import { LoopalSessionDirectory } from '../sessions/loopal-session-directory'
import {
  type SessionRuntimeNotificationEvent,
} from '../runtime/session-runtime-registry'
import {
  type AgentControlOperations,
  LoopalAgentControl,
} from '../attention/loopal-agent-control'
import {
  bindLoopalSettings,
  type LoopalSettingsOperations,
  LoopalSettingsService,
} from '../settings/loopal-settings-service'
import {
  bindLoopalMcpSettings, LoopalMcpSettingsService, type LoopalMcpSettingsOperations,
} from '../settings/loopal-mcp-settings-service'
import {
  bindLoopalSkillPlugins, LoopalSkillPluginService, type LoopalSkillPluginOperations,
} from '../settings/loopal-skill-plugin-service'

export class LoopalBackendServices {
  readonly code: LoopalCodeWorkbench
  readonly agent: LoopalAgentControl
  readonly settings: LoopalSettingsService
  readonly mcpSettings: LoopalMcpSettingsService
  readonly skillPlugins: LoopalSkillPluginService

  constructor(
    router: CodeWorkbenchRuntimeRouter,
    private readonly directory: LoopalSessionDirectory,
    private readonly fire: (event: DesktopEvent) => void,
  ) {
    this.code = new LoopalCodeWorkbench(router)
    this.settings = new LoopalSettingsService(router)
    this.mcpSettings = new LoopalMcpSettingsService(router)
    this.skillPlugins = new LoopalSkillPluginService(router)
    this.agent = new LoopalAgentControl({
      session: (sessionId) => {
        const runtime = this.directory.runtimeForSession(sessionId)
        return runtime ? { runtime } : undefined
      },
    })
  }

  operations(): CodeWorkbenchOperations & AgentControlOperations & LoopalSettingsOperations
    & LoopalMcpSettingsOperations & LoopalSkillPluginOperations {
    return {
      ...bindCodeWorkbench(this.code),
      ...bindLoopalSettings(this.settings),
      ...bindLoopalMcpSettings(this.mcpSettings),
      ...bindLoopalSkillPlugins(this.skillPlugins),
      interruptAgent: this.agent.interruptAgent.bind(this.agent),
      controlAgent: this.agent.controlAgent.bind(this.agent),
    }
  }

  accept(event: SessionRuntimeNotificationEvent): void {
    if (!this.directory.leaders.isLeader(event.runtimeId, event.workspaceId)) return
    const projected = projectCodeWorkbenchEvent(event.method, event.params)
    if (projected) this.fire(projected)
  }
}
