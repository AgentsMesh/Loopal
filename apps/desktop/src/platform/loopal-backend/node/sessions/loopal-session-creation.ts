import { type CreateSessionInput, type SessionDetail } from '../../../../shared/contracts'
import { LoopalSessionDirectory } from './loopal-session-directory'
import { mergeRuntimeCatalog } from '../backend/loopal-backend-lifecycle'
import { LoopalSessionWorkspaces } from './loopal-session-workspaces'
import { type WorkspaceCatalogStage } from '../workspace/loopal-workspace-catalog'
import { SessionDirectoryAuthority } from './session-directory-authority'
import {
  type SessionRuntimeHandle, SessionRuntimeRegistry,
} from '../runtime/session-runtime-registry'

interface SessionCreationServices {
  readonly directories: SessionDirectoryAuthority
  readonly registry: SessionRuntimeRegistry
  readonly directory: LoopalSessionDirectory
  readonly workspaces: LoopalSessionWorkspaces
}

export async function createLoopalSession(
  input: CreateSessionInput,
  services: SessionCreationServices,
): Promise<SessionDetail> {
  const claim = await services.directories.claim(input)
  let runtime: SessionRuntimeHandle | undefined
  let activatedSessionId: string | undefined
  let workspaceStage: WorkspaceCatalogStage | undefined
  try {
    const stage = services.workspaces.stage(claim.target)
    workspaceStage = stage
    const workspace = stage.workspace
    runtime = await services.registry.startFresh({
      workspaceId: workspace.id, cwd: claim.target.path,
    }, async (sessionId) => {
      if (activatedSessionId && activatedSessionId !== sessionId) {
        throw new Error('Desktop Host changed Session during creation')
      }
      activatedSessionId = sessionId
      claim.commit()
      stage.commit()
      await services.workspaces.created(sessionId, workspace.id)
    })
    await services.workspaces.started(runtime, undefined, true)
    const state = await services.directory.attach(runtime, true)
    await mergeRuntimeCatalog(services.directory, runtime, true)
    await services.workspaces.started(runtime, state.detail.session, true)
    return state.detail
  } catch (error) {
    if (activatedSessionId) {
      if (runtime) await services.registry.stop(runtime.runtimeId).catch(() => undefined)
      await services.workspaces.stopped(activatedSessionId).catch(() => undefined)
      throw recoveryError(error, activatedSessionId, claim.target.path)
    }
    if (isCommitUnknown(error)) {
      claim.commit()
      workspaceStage?.commit()
      throw unknownCreationError(error, claim.target)
    }
    try {
      await claim.rollback()
    } catch (rollbackError) {
      throw retainedWorktreeError(error, rollbackError, claim.target.path)
    }
    throw error
  }
}

function recoveryError(error: unknown, sessionId: string, path: string): Error {
  return new Error(
    `session_created_recovery_required: Loopal session ${sessionId} was created at ${path}; `
      + `Desktop initialization failed (${message(error)})`,
    { cause: error },
  )
}

function retainedWorktreeError(
  creationError: unknown,
  rollbackError: unknown,
  path: string,
): Error {
  const error = new Error(
    `worktree_retained: session creation failed (${message(creationError)}); `
      + `cleanup failed and the worktree was retained at ${path} (${message(rollbackError)})`,
    { cause: new AggregateError([creationError, rollbackError]) },
  )
  return error
}

function unknownCreationError(
  error: unknown,
  target: { path: string; kind: 'folder' | 'git_worktree' },
): Error {
  const retained = target.kind === 'git_worktree' ? 'worktree_retained' : 'directory_retained'
  return new Error(
    `${retained}: session_creation_state_unknown; retained ${target.path} for recovery `
      + `(${message(error)})`,
    { cause: error },
  )
}

function isCommitUnknown(value: unknown): boolean {
  if (value instanceof AggregateError) return value.errors.some(isCommitUnknown)
  if (!(value instanceof Error)) return false
  return value.message.includes('desktop_protocol_drain_incomplete')
    || value.message.includes('desktop_process_termination_unconfirmed')
    || value.message.includes('desktop_session_creation_state_unknown')
    || isCommitUnknown(value.cause)
}

function message(value: unknown): string {
  return value instanceof Error ? value.message : String(value)
}
