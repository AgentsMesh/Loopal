import { type CodeWorkbenchOperations } from '../workspace/loopal-code-workbench'
import { type AgentControlOperations } from '../attention/loopal-agent-control'
import { type FakeMetaHubOperations } from './fake-metahub'
import { type LoopalSettingsOperations } from './fake-loopal-settings'
import { type LoopalMcpSettingsOperations } from '../settings/loopal-mcp-settings-service'
import { type DesktopPreferencesOperations } from '../settings/desktop-preferences-service'
import { type LoopalSkillPluginOperations } from '../settings/loopal-skill-plugin-service'

export interface FakeBackendClock {
  now(): Date
  delay(milliseconds: number): Promise<void>
}

export const systemClock: FakeBackendClock = {
  now: () => new Date(),
  delay: (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds)),
}

export interface FakeBackendOperations extends
  CodeWorkbenchOperations,
  AgentControlOperations,
  FakeMetaHubOperations,
  LoopalSettingsOperations,
  LoopalMcpSettingsOperations,
  LoopalSkillPluginOperations,
  DesktopPreferencesOperations {}
