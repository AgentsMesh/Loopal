import { loadSessionCatalog } from '../sessions/loopal-session-catalog'
import { LoopalSessionDirectory } from '../sessions/loopal-session-directory'
import { LoopalSessionResumeState } from '../sessions/loopal-session-resume-state'
import {
  type SessionRuntimeHandle,
  SessionRuntimeRegistry,
} from '../runtime/session-runtime-registry'

interface BootstrapRuntimeInput {
  readonly registry: SessionRuntimeRegistry
  readonly directory: LoopalSessionDirectory
  readonly resumeState: LoopalSessionResumeState
  readonly workspaceId: string
  readonly cwd: string
  readonly resumeSessionId?: string
}

export async function bootstrapRuntime(input: BootstrapRuntimeInput): Promise<{
  runtime: SessionRuntimeHandle
  activeSessionId: string
}> {
  const requested = input.resumeSessionId
  const runtime = await startInitial(input, requested)
  await mergeRuntimeCatalog(input.directory, runtime, false)
  await input.directory.attach(runtime, false)
  const active = input.resumeState.activeSessionId
  const activeSessionId = active && input.directory.session(active) ? active : runtime.sessionId
  await input.resumeState.normalizeRunning([runtime.sessionId], activeSessionId)
  return { runtime, activeSessionId }
}

export async function mergeRuntimeCatalog(
  directory: LoopalSessionDirectory,
  runtime: SessionRuntimeHandle,
  emit: boolean,
): Promise<void> {
  const catalog = await loadSessionCatalog(runtime.host, runtime.workspaceId)
  directory.mergeCatalog(catalog, runtime.workspaceId, emit)
}

async function startInitial(
  input: BootstrapRuntimeInput,
  resumeSessionId?: string,
): Promise<SessionRuntimeHandle> {
  const workspace = { workspaceId: input.workspaceId, cwd: input.cwd }
  if (!resumeSessionId) return input.registry.startFresh(workspace)
  try {
    return await input.registry.resume({ ...workspace, sessionId: resumeSessionId })
  } catch {
    await input.resumeState.stopped(resumeSessionId)
    return input.registry.startFresh(workspace)
  }
}

export function unknownSession(sessionId: string): Error {
  return new Error(`Unknown Loopal Desktop session: ${sessionId}`)
}

export function stoppedSession(sessionId: string): Error {
  return new Error(`Session is not running; restart it first: ${sessionId}`)
}

export function archivedSession(sessionId: string): Error {
  return new Error(`Archived session cannot be restarted: ${sessionId}`)
}
