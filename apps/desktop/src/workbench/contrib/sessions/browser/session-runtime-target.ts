import {
  type MetaHubRuntimeTarget, type RuntimeSummary, type SessionSummary,
} from '../../../../shared/contracts'
import { isSessionLive } from '../../../../shared/contracts/session-lifecycle'

export function resolveMetaHubRuntimeTarget(
  sessionId: string,
  sessions: readonly SessionSummary[],
  runtimes: readonly RuntimeSummary[],
): MetaHubRuntimeTarget | undefined {
  const session = sessions.find((candidate) => candidate.id === sessionId)
  if (!session?.activeRuntimeId || !isSessionLive(session)) return undefined
  const runtime = runtimes.find((candidate) => candidate.id === session.activeRuntimeId)
  if (!runtime || runtime.state !== 'ready'
    || runtime.sessionId !== session.id
    || runtime.workspaceId !== session.workspaceId) return undefined
  return { sessionId, runtimeId: runtime.id, generation: runtime.generation }
}
