import { CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import { Emitter } from '../../../../base/common/event'
import { type IDisposable } from '../../../../base/common/lifecycle'
import {
  type Artifact, type ConversationEntry, type CreateSessionInput, type DesktopEvent,
  type DesktopImageAttachment, type RuntimeSummary, type SessionDetail, type WorkbenchBootstrap,
  type SessionDirectorySelection,
} from '../../../../shared/contracts'
import { canRestartSession, isSessionLive } from '../../../../shared/contracts/session-lifecycle'
import { type DesktopBackend } from '../../common/backend'
import { bindFakeCodeWorkbench } from './fake-code-workbench'
import { createFakeSessionCatalog } from './fake-session-fixtures'
import { bindFakeAgentControl } from './fake-agent-control'
import { appendFakeAgentMessage, fakeProducerAgent } from './fake-message-target'
import { bindFakeMetaHub } from './fake-metahub'
import { bindFakeLoopalSettings } from './fake-loopal-settings'
import { bindFakeMcpSettings } from './fake-mcp-settings'
import { bindFakeSkillPlugins } from './fake-skill-plugin-settings'
import { bindDesktopPreferences, DesktopPreferencesService } from '../settings/desktop-preferences-service'
import {
  type FakeBackendClock, type FakeBackendOperations, systemClock,
} from './fake-backend-types'
import { FakeSessionDirectoryAuthority } from './fake-session-directory'
import { LoopalWorkspaceCatalog } from '../workspace/loopal-workspace-catalog'
export { type FakeBackendClock } from './fake-backend-types'
export interface FakeDesktopBackend extends FakeBackendOperations {}
export class FakeDesktopBackend implements DesktopBackend, IDisposable {
  private readonly emitter = new Emitter<DesktopEvent>()
  private readonly catalog
  private sequence = 100
  private readonly directories = new FakeSessionDirectoryAuthority()
  private readonly workspaces: LoopalWorkspaceCatalog
  readonly onEvent = this.emitter.event
  constructor(
    private readonly clock: FakeBackendClock = systemClock,
    desktopPreferencesPath?: string,
  ) {
    this.catalog = createFakeSessionCatalog(this.iso())
    this.workspaces = new LoopalWorkspaceCatalog(this.catalog.workspace)
    Object.assign(this, bindFakeCodeWorkbench(
      this.catalog.workspace.id,
      (event) => this.emitter.fire(event),
    ))
    Object.assign(this, bindFakeAgentControl(
      this.catalog, () => this.iso(), (event) => this.emitter.fire(event),
    ))
    Object.assign(this, bindFakeLoopalSettings(this.catalog.workspace.id))
    Object.assign(this, bindFakeMcpSettings(this.catalog.workspace.id))
    Object.assign(this, bindFakeSkillPlugins(this.catalog.workspace.id))
    Object.assign(this, bindDesktopPreferences(
      new DesktopPreferencesService(desktopPreferencesPath),
    ))
    Object.assign(this, bindFakeMetaHub((target) => {
      const runtime = this.catalog.runtimes.get(target.runtimeId)
      return this.catalog.details.get(target.sessionId)?.session.activeRuntimeId === target.runtimeId
        && runtime?.sessionId === target.sessionId
        && runtime.generation === target.generation
        && runtime.state === 'ready'
    }, (target, state) => {
      const detail = this.catalog.details.get(target.sessionId)
      if (!detail) return
      detail.metaHub = structuredClone(state)
      this.emitter.fire({ type: 'session_detail_replaced', detail: structuredClone(detail) })
    }))
  }
  async bootstrap(token = CancellationToken.None): Promise<WorkbenchBootstrap> {
    throwIfCancelled(token)
    return {
      protocolVersion: 2,
      hostStatus: 'ready',
      workspaces: [...this.workspaces.values()],
      sessions: [...this.catalog.details.values()].map(({ session }) => ({ ...session })),
      runtimes: [...this.catalog.runtimes.values()].map((runtime) => ({ ...runtime })),
      activeSessionId: 'session-desktop',
    }
  }
  async openSession(
    sessionId: string, token = CancellationToken.None,
  ): Promise<SessionDetail> {
    throwIfCancelled(token)
    return structuredClone(this.requireDetail(sessionId))
  }
  async createSession(
    input: CreateSessionInput, token = CancellationToken.None,
  ): Promise<SessionDetail> {
    throwIfCancelled(token)
    const target = await this.directories.prepare(input)
    const workspace = this.workspaces.ensure(target.path, target.kind)
    const id = `session-${this.sequence++}`
    const runtime = this.newRuntime(id, workspace.id, 1)
    const detail: SessionDetail = {
      session: {
        id, workspaceId: workspace.id, title: 'New session', model: 'gpt-5', mode: 'agent',
        status: 'running', createdAt: this.iso(), updatedAt: this.iso(),
        activeRuntimeId: runtime.id,
      },
      conversation: [], agents: [{ id: runtime.rootAgent, name: 'Loopal', status: 'running' }], artifacts: [],
    }
    this.catalog.details.set(id, detail)
    this.catalog.runtimes.set(runtime.id, runtime)
    this.emitter.fire({ type: 'session_updated', session: { ...detail.session } })
    this.emitter.fire({ type: 'runtime_updated', runtime: { ...runtime } })
    return structuredClone(detail)
  }
  authorizeSessionDirectory(path: string): Promise<SessionDirectorySelection> {
    return this.directories.authorize(path)
  }
  async stopSession(sessionId: string, token = CancellationToken.None): Promise<void> {
    throwIfCancelled(token)
    const detail = this.requireDetail(sessionId)
    const runtime = this.activeRuntime(detail)
    if (runtime) this.updateRuntime(runtime, 'stopped')
    const { activeRuntimeId: _active, ...session } = detail.session
    detail.session = { ...session, status: 'stopped', updatedAt: this.iso() }
    this.publishSession(detail)
  }
  async restartSession(
    sessionId: string, token = CancellationToken.None,
  ): Promise<RuntimeSummary> {
    throwIfCancelled(token)
    const detail = this.requireDetail(sessionId)
    if (!canRestartSession(detail.session)) throw new Error(`Archived session cannot be restarted: ${sessionId}`)
    const active = this.activeRuntime(detail)
    if (active) this.updateRuntime(active, 'stopped')
    const generation = Math.max(0, ...[...this.catalog.runtimes.values()]
      .filter((runtime) => runtime.sessionId === sessionId)
      .map((runtime) => runtime.generation)) + 1
    const runtime = this.newRuntime(sessionId, detail.session.workspaceId, generation)
    this.catalog.runtimes.set(runtime.id, runtime)
    detail.session = {
      ...detail.session, activeRuntimeId: runtime.id,
      status: 'running', updatedAt: this.iso(), attention: undefined,
    }
    this.emitter.fire({ type: 'runtime_updated', runtime: { ...runtime } })
    this.publishSession(detail)
    return { ...runtime }
  }
  async sendMessage(
    sessionId: string, text: string,
    token = CancellationToken.None, agentId = 'main',
    images: readonly DesktopImageAttachment[] = [],
  ): Promise<void> {
    throwIfCancelled(token)
    const detail = this.requireDetail(sessionId)
    if (!isSessionLive(detail.session)) throw new Error(`Session is not running; restart it first: ${sessionId}`)
    const publish = (entry: ConversationEntry): void => {
      if (appendFakeAgentMessage(detail, agentId, entry)) this.publishEntry(detail, entry)
      else this.emitter.fire({ type: 'session_detail_replaced', detail: structuredClone(detail) })
    }
    publish({ ...this.entry('user', text), ...(images.length ? { imageCount: images.length } : {}) })
    await this.clock.delay(15)
    throwIfCancelled(token)
    publish(this.entry(
      'assistant', 'Loopal handled this message inside the selected session runtime.',
    ))
    const artifact: Artifact = {
      id: `artifact-${this.sequence++}`, sessionId, title: 'Execution summary.md',
      kind: 'report', uri: `loopal-artifact://${sessionId}/execution-summary.md`,
      mediaType: 'text/markdown', producerAgentId: fakeProducerAgent(detail, agentId),
      createdAt: this.iso(),
    }
    detail.artifacts.push(artifact)
    detail.session = { ...detail.session, updatedAt: this.iso() }
    this.emitter.fire({ type: 'artifact_created', artifact })
    this.publishSession(detail)
  }
  dispose(): void { this.emitter.dispose() }
  private requireDetail(sessionId: string): SessionDetail {
    const detail = this.catalog.details.get(sessionId)
    if (!detail) throw new Error(`Session not found: ${sessionId}`)
    return detail
  }
  private activeRuntime(detail: SessionDetail): RuntimeSummary | undefined {
    return detail.session.activeRuntimeId ? this.catalog.runtimes.get(
      detail.session.activeRuntimeId,
    ) : undefined
  }
  private newRuntime(sessionId: string, workspaceId: string, generation: number): RuntimeSummary {
    return {
      id: `${sessionId}-runtime-${generation}`, sessionId, workspaceId, generation,
      state: 'ready', rootAgent: 'agent-root', startedAt: this.iso(),
    }
  }
  private updateRuntime(runtime: RuntimeSummary, state: RuntimeSummary['state']): void {
    const next = { ...runtime, state }
    this.catalog.runtimes.set(next.id, next)
    this.emitter.fire({ type: 'runtime_updated', runtime: next })
  }
  private publishSession(detail: SessionDetail): void {
    this.emitter.fire({ type: 'session_updated', session: { ...detail.session } })
  }
  private publishEntry(detail: SessionDetail, entry: ConversationEntry): void {
    detail.conversation.push(entry)
    this.emitter.fire({ type: 'conversation_entry', sessionId: detail.session.id, entry })
  }
  private entry(role: ConversationEntry['role'], text: string): ConversationEntry {
    return { id: `message-${this.sequence++}`, role, text, createdAt: this.iso() }
  }
  private iso(): string { return this.clock.now().toISOString() }
}
