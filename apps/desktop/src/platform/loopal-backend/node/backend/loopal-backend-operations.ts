import { type AgentControlOperations } from '../attention/loopal-agent-control'
import { type DesktopPreferencesOperations } from '../settings/desktop-preferences-service'
import { type CodeWorkbenchOperations } from '../workspace/loopal-code-workbench'
import { type LoopalMcpSettingsOperations } from '../settings/loopal-mcp-settings-service'
import { type MetaHubBackendOperations } from '../federation/loopal-metahub-runtime'
import { type LoopalSettingsOperations } from '../settings/loopal-settings-service'
import { type LoopalSkillPluginOperations } from '../settings/loopal-skill-plugin-service'

export interface LoopalBackendOperations extends
  CodeWorkbenchOperations,
  AgentControlOperations,
  MetaHubBackendOperations,
  LoopalSettingsOperations,
  LoopalMcpSettingsOperations,
  LoopalSkillPluginOperations,
  DesktopPreferencesOperations {}
