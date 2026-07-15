import { basename } from 'node:path'
import { pathToFileURL } from 'node:url'
import { type CancellationToken } from '../../../../base/common/cancellation'
import { Emitter } from '../../../../base/common/event'
import { Disposable } from '../../../../base/common/lifecycle'
import {
  type CreateSessionInput, type DesktopEvent, type DesktopImageAttachment, type RuntimeSummary,
  type SessionDetail, type WorkbenchBootstrap, type Workspace,
} from '../../../../shared/contracts'
import { canRestartSession } from '../../../../shared/contracts/session-lifecycle'
import { type DesktopBackend } from '../../common/backend'
import { createBackendRegistry } from './loopal-backend-registry'
import { LoopalBackendServices } from './loopal-backend-services'
import { type DesktopHostClient, type LoopalDesktopBackendOptions } from './loopal-backend-types'
import { bindDesktopPreferences, DesktopPreferencesService } from '../settings/desktop-preferences-service'
import { LoopalMetaHubRuntime } from '../federation/loopal-metahub-runtime'
import { type LoopalBackendOperations } from './loopal-backend-operations'
import {
  archivedSession, bootstrapRuntime, mergeRuntimeCatalog, stoppedSession, unknownSession,
} from './loopal-backend-lifecycle'
import { LoopalSessionDirectory } from '../sessions/loopal-session-directory'
import { type SessionRuntimeHandle, SessionRuntimeRegistry } from '../runtime/session-runtime-registry'
import { SessionDirectoryAuthority } from '../sessions/session-directory-authority'
import { LoopalSessionWorkspaces } from '../sessions/loopal-session-workspaces'
import { backendSnapshot } from './loopal-backend-snapshot'
import { createSessionDirectoryCommand } from '../sessions/session-directory-command'
import { createLoopalSession } from '../sessions/loopal-session-creation'
export type { DesktopHostClient, LoopalDesktopBackendOptions } from './loopal-backend-types'
export interface LoopalDesktopBackend extends LoopalBackendOperations {}
export class LoopalDesktopBackend extends Disposable implements DesktopBackend {
  private readonly events = this.register(new Emitter<DesktopEvent>())
  private readonly registry: SessionRuntimeRegistry
  private readonly directory: LoopalSessionDirectory
  private readonly services: LoopalBackendServices
  private readonly now: () => Date
  private readonly workspaces: LoopalSessionWorkspaces
  private readonly sessionDirectories: SessionDirectoryAuthority
  private readonly metaHub: LoopalMetaHubRuntime
  private readonly preferences: DesktopPreferencesService
  private bootstrapping?: Promise<WorkbenchBootstrap>
  private bootstrapped = false
  private activeSessionId?: string
  private hostStatus: DesktopHostClient['currentStatus'] = 'stopped'
  readonly onEvent = this.events.event
  constructor(private readonly options: LoopalDesktopBackendOptions) {
    super()
    this.now = options.now ?? (() => new Date())
    const workspace: Workspace = {
      id: 'local-workspace',
      name: basename(options.cwd) || 'Workspace',
      rootUri: pathToFileURL(options.cwd).href,
      kind: 'folder',
    }
    this.workspaces = new LoopalSessionWorkspaces(workspace, options.cwd, options.sessionStatePath)
    this.metaHub = this.register(new LoopalMetaHubRuntime({
      binaryPath: options.binaryPath, parentPid: options.parentPid ?? process.pid,
      ...(options.metaHubSettingsPath ? { settingsPath: options.metaHubSettingsPath } : {}),
    }))
    this.preferences = new DesktopPreferencesService(options.desktopPreferencesPath)
    this.registry = this.register(createBackendRegistry({
      ...options,
      getMetaHubStartup: () => this.metaHub.startup,
    }))
    this.directory = this.register(new LoopalSessionDirectory(
      this.registry,
      this.now,
      workspace.name,
      {
        event: (event) => {
          if (event.type === 'host_status') this.hostStatus = event.status
          if (event.type === 'runtime_updated' && event.runtime.state === 'crashed') {
            void this.workspaces.stopped(event.runtime.sessionId).catch(() => undefined)
          }
          this.events.fire(event)
        },
        service: (event) => this.services.accept(event),
      },
    ))
    this.sessionDirectories = new SessionDirectoryAuthority(
      options.sessionDirectoryRequest ?? createSessionDirectoryCommand(options.binaryPath),
    )
    this.services = new LoopalBackendServices({
      workspace: (workspaceId) => this.workspaceRuntime(workspaceId),
      liveSession: (sessionId) => this.liveSessionRuntime(sessionId),
    }, this.directory, (event) => this.events.fire(event))
    Object.assign(this, this.services.operations())
    Object.assign(this, this.metaHub.operations(this.directory, this.now))
    Object.assign(this, bindDesktopPreferences(this.preferences))
  }
  bootstrap(): Promise<WorkbenchBootstrap> {
    if (this.bootstrapped) return Promise.resolve(this.snapshot())
    this.bootstrapping ??= this.bootstrapInner()
    return this.bootstrapping
  }
  async openSession(sessionId: string): Promise<SessionDetail> {
    await this.ensureBootstrapped()
    const live = this.directory.liveSession(sessionId)
    if (live) await live.initialize()
    const detail = this.directory.detail(sessionId)
    if (!detail) throw unknownSession(sessionId)
    this.activeSessionId = sessionId
    await this.workspaces.select(sessionId)
    return detail
  }
  async createSession(input: CreateSessionInput): Promise<SessionDetail> {
    await this.ensureBootstrapped()
    const detail = await createLoopalSession(input, {
      directories: this.sessionDirectories, registry: this.registry,
      directory: this.directory, workspaces: this.workspaces,
    })
    this.activeSessionId = detail.session.id
    return detail
  }
  async authorizeSessionDirectory(path: string) {
    return this.sessionDirectories.authorize(path)
  }
  async stopSession(sessionId: string): Promise<void> {
    await this.ensureBootstrapped()
    if (!this.directory.session(sessionId)) throw unknownSession(sessionId)
    await this.workspaces.stopped(sessionId)
    const runtime = this.directory.runtimeForSession(sessionId)
    if (!runtime) return
    await this.registry.stop(runtime.runtimeId)
  }
  async restartSession(sessionId: string): Promise<RuntimeSummary> {
    await this.ensureBootstrapped()
    const session = this.directory.session(sessionId)
    if (!session) throw unknownSession(sessionId)
    if (!canRestartSession(session)) throw archivedSession(sessionId)
    const current = this.directory.runtimeForSession(sessionId)
    const runtime = current
      ? await this.registry.restart(current.runtimeId)
      : await this.registry.resume({
        workspaceId: session.workspaceId,
        cwd: this.workspaces.cwd(sessionId, session.workspaceId),
        sessionId,
      })
    await this.directory.attach(runtime, true)
    this.activeSessionId = sessionId
    await this.workspaces.started(runtime, this.directory.session(sessionId), true)
    return this.directory.runtime(runtime.runtimeId)!
  }
  async sendMessage(
    sessionId: string, text: string, _token?: CancellationToken, agentId = 'main',
    images: readonly DesktopImageAttachment[] = [],
  ): Promise<void> {
    await this.ensureBootstrapped()
    const state = this.directory.liveSession(sessionId)
    if (!state) {
      if (!this.directory.session(sessionId)) throw unknownSession(sessionId)
      throw stoppedSession(sessionId)
    }
    await state.send(text, agentId, images)
  }
  async shutdown(): Promise<void> {
    await Promise.all([this.workspaces.flush(), this.metaHub.flush(), this.preferences.flush()])
    try { await this.registry.shutdownAll() }
    finally { await this.metaHub.stop() }
  }
  private async bootstrapInner(): Promise<WorkbenchBootstrap> {
    await Promise.all([this.metaHub.load(), this.preferences.getDesktopPreferences()])
    await this.workspaces.load(this.directory)
    const resume = this.workspaces.bootstrapTarget
    const { runtime, activeSessionId } = await bootstrapRuntime({
      registry: this.registry, directory: this.directory, resumeState: this.workspaces.lifecycle,
      workspaceId: resume.workspaceId, cwd: resume.cwd,
      ...(resume.resumeSessionId ? { resumeSessionId: resume.resumeSessionId } : {}),
    })
    this.activeSessionId = activeSessionId
    this.hostStatus = runtime.host.currentStatus
    this.bootstrapped = true
    return this.snapshot()
  }
  private snapshot(): WorkbenchBootstrap {
    return backendSnapshot({
      hostStatus: this.hostStatus, workspaces: this.workspaces.values(),
      sessions: this.directory.sessionValues(), runtimes: this.directory.runtimeValues(),
      ...(this.activeSessionId ? { activeSessionId: this.activeSessionId } : {}),
    })
  }
  private async workspaceRuntime(workspaceId: string): Promise<SessionRuntimeHandle> {
    await this.ensureBootstrapped()
    this.workspaces.require(workspaceId)
    const leader = this.directory.leaders.current(workspaceId)
    if (leader) return leader
    throw new Error(`Workspace has no live runtime; restart a session: ${workspaceId}`)
  }
  private async liveSessionRuntime(sessionId: string): Promise<SessionRuntimeHandle | undefined> {
    await this.ensureBootstrapped()
    return this.directory.runtimeForSession(sessionId)
  }
  private async ensureBootstrapped(): Promise<void> { if (!this.bootstrapped) await this.bootstrap() }
}
