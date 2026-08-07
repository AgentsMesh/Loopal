import { CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import { Emitter } from '../../../../base/common/event'
import { type IDisposable } from '../../../../base/common/lifecycle'
import {
  type AgentControlDisposition,
  type AgentControlInput,
  type AgentControlTarget,
  type CreateSessionInput,
  type DesktopImageAttachment,
  type DesktopEvent,
  type RuntimeSummary,
  type SessionDetail,
  type SessionDirectorySelection,
  type WorkbenchBootstrap,
} from '../../../../shared/contracts'
import { type DesktopBackend } from '../../common/backend'
import { type CodeWorkbenchOperations } from '../workspace/loopal-code-workbench'
import { bindUnavailableCodeWorkbench } from './unavailable-code-workbench'
import { bindUnavailableMetaHub } from './unavailable-metahub'
import { type FakeMetaHubOperations } from '../fake/fake-metahub'
import { bindUnavailableLoopalSettings } from '../fake/fake-loopal-settings'
import { type LoopalSettingsOperations } from '../settings/loopal-settings-service'
import { bindUnavailableMcpSettings } from '../fake/fake-mcp-settings'
import { type LoopalMcpSettingsOperations } from '../settings/loopal-mcp-settings-service'
import { bindUnavailableSkillPlugins } from '../fake/fake-skill-plugin-settings'
import { type LoopalSkillPluginOperations } from '../settings/loopal-skill-plugin-service'
import {
  bindDesktopPreferences, DesktopPreferencesService, type DesktopPreferencesOperations,
} from '../settings/desktop-preferences-service'

export interface UnavailableDesktopBackend extends CodeWorkbenchOperations, FakeMetaHubOperations,
  LoopalSettingsOperations, LoopalMcpSettingsOperations, LoopalSkillPluginOperations,
  DesktopPreferencesOperations {}
export class UnavailableDesktopBackend implements DesktopBackend, IDisposable {
  private readonly emitter = new Emitter<DesktopEvent>()
  readonly onEvent = this.emitter.event

  constructor(private readonly reason: string) {
    Object.assign(this, bindUnavailableCodeWorkbench(reason))
    Object.assign(this, bindUnavailableMetaHub(reason))
    Object.assign(this, bindUnavailableLoopalSettings(reason))
    Object.assign(this, bindUnavailableMcpSettings(reason))
    Object.assign(this, bindUnavailableSkillPlugins(reason))
    Object.assign(this, bindDesktopPreferences(new DesktopPreferencesService()))
  }

  async bootstrap(token = CancellationToken.None): Promise<WorkbenchBootstrap> {
    throwIfCancelled(token)
    return {
      protocolVersion: 2,
      hostStatus: 'stopped',
      workspaces: [],
      sessions: [],
      runtimes: [],
    }
  }

  async openSession(_sessionId: string, token = CancellationToken.None): Promise<SessionDetail> {
    throwIfCancelled(token)
    throw new Error(this.reason)
  }

  async createSession(
    _input: CreateSessionInput,
    token = CancellationToken.None,
  ): Promise<SessionDetail> {
    throwIfCancelled(token)
    throw new Error(this.reason)
  }

  async authorizeSessionDirectory(_path: string): Promise<SessionDirectorySelection> {
    throw new Error(this.reason)
  }

  async stopSession(_sessionId: string, token = CancellationToken.None): Promise<void> {
    throwIfCancelled(token)
    throw new Error(this.reason)
  }

  async restartSession(
    _sessionId: string,
    token = CancellationToken.None,
  ): Promise<RuntimeSummary> {
    throwIfCancelled(token)
    throw new Error(this.reason)
  }

  async sendMessage(
    _sessionId: string,
    _text: string,
    token = CancellationToken.None,
    _agentId?: string,
    _images?: readonly DesktopImageAttachment[],
  ): Promise<void> {
    throwIfCancelled(token)
    throw new Error(this.reason)
  }

  async interruptAgent(
    _input: AgentControlTarget,
    token = CancellationToken.None,
  ): Promise<void> {
    throwIfCancelled(token)
    throw new Error(this.reason)
  }

  async controlAgent(
    _input: AgentControlInput,
    token = CancellationToken.None,
  ): Promise<AgentControlDisposition> {
    throwIfCancelled(token)
    throw new Error(this.reason)
  }

  dispose(): void {
    this.emitter.dispose()
  }
}
