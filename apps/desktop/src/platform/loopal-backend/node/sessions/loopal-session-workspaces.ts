import { realpath, stat } from 'node:fs/promises'
import { type SessionSummary, type Workspace } from '../../../../shared/contracts'
import { LoopalSessionDirectory } from './loopal-session-directory'
import {
  LoopalSessionResumeState, type SessionLocation,
} from './loopal-session-resume-state'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'
import { LoopalWorkspaceCatalog, type WorkspaceCatalogStage } from '../workspace/loopal-workspace-catalog'
import { type PreparedSessionDirectory } from './session-directory-authority'

export class LoopalSessionWorkspaces {
  private readonly catalog: LoopalWorkspaceCatalog
  private readonly resume: LoopalSessionResumeState

  constructor(readonly initial: Workspace, cwd: string, statePath?: string) {
    this.catalog = new LoopalWorkspaceCatalog(initial)
    this.resume = new LoopalSessionResumeState(cwd, statePath)
  }

  async load(directory: LoopalSessionDirectory): Promise<void> {
    await this.resume.load()
    const unavailable: string[] = []
    for (const location of this.resume.locations) {
      this.catalog.restore(location)
      restoreSession(directory, location)
      if (!(await isAvailable(location.cwd))) {
        directory.markSessionUnavailable(location.sessionId)
        unavailable.push(location.sessionId)
      }
    }
    if (unavailable.length > 0) await this.resume.discardUnavailable(unavailable)
  }

  get bootstrapTarget(): { workspaceId: string; cwd: string; resumeSessionId?: string } {
    const sessionId = this.resume.resumeSessionId
    const location = sessionId ? this.resume.location(sessionId) : undefined
    return location
      ? {
        workspaceId: location.workspaceId, cwd: location.cwd,
        resumeSessionId: location.sessionId,
      }
      : {
        workspaceId: this.initial.id, cwd: this.catalog.path(this.initial.id)!,
        ...(sessionId ? { resumeSessionId: sessionId } : {}),
      }
  }

  ensure(target: PreparedSessionDirectory): Workspace {
    return this.catalog.ensure(target.path, target.kind)
  }

  stage(target: PreparedSessionDirectory): WorkspaceCatalogStage {
    return this.catalog.stage(target.path, target.kind)
  }

  require(workspaceId: string): Workspace {
    const workspace = this.catalog.get(workspaceId)
    if (!workspace) throw new Error(`Unknown workspace: ${workspaceId}`)
    return workspace
  }

  cwd(sessionId: string, workspaceId: string): string {
    return this.resume.location(sessionId)?.cwd
      ?? this.catalog.path(workspaceId)
      ?? this.catalog.path(this.initial.id)!
  }

  started(
    runtime: SessionRuntimeHandle, session: SessionSummary | undefined, select: boolean,
  ): Promise<void> {
    return this.resume.started(
      runtime.sessionId, select, this.location(runtime.sessionId, runtime.workspaceId, session),
    )
  }

  created(sessionId: string, workspaceId: string): Promise<void> {
    return this.resume.created(this.location(sessionId, workspaceId))
  }

  private location(
    sessionId: string, workspaceId: string, session?: SessionSummary,
  ): SessionLocation {
    const workspace = this.require(workspaceId)
    return {
      sessionId,
      workspaceId: workspace.id,
      cwd: this.catalog.path(workspace.id)!,
      name: workspace.name,
      kind: normalizedKind(workspace),
      ...(session ? summaryFields(session) : {}),
    }
  }

  select(sessionId: string): Promise<void> { return this.resume.select(sessionId) }
  stopped(sessionId: string): Promise<void> { return this.resume.stopped(sessionId) }
  flush(): Promise<void> { return this.resume.flush() }
  values(): readonly Workspace[] { return this.catalog.values() }
  get lifecycle(): LoopalSessionResumeState { return this.resume }
}

function normalizedKind(workspace: Workspace): 'folder' | 'git_worktree' {
  return workspace.kind === 'git_worktree' ? 'git_worktree' : 'folder'
}

function summaryFields(session: SessionSummary) {
  return {
    title: session.title, model: session.model, mode: session.mode,
    createdAt: session.createdAt, updatedAt: session.updatedAt,
  }
}

function restoreSession(directory: LoopalSessionDirectory, location: SessionLocation): void {
  const timestamp = location.updatedAt ?? location.createdAt ?? new Date(0).toISOString()
  directory.mergeCatalog([{
    id: location.sessionId,
    title: location.title ?? `Loopal session · ${location.name}`,
    model: location.model ?? 'loopal-default', mode: location.mode ?? 'agent',
    createdAt: location.createdAt ?? timestamp, updatedAt: timestamp,
  }], location.workspaceId, false)
}

async function isAvailable(path: string): Promise<boolean> {
  try {
    const [resolved, metadata] = await Promise.all([realpath(path), stat(path)])
    return metadata.isDirectory() && resolved === path
  } catch {
    return false
  }
}
