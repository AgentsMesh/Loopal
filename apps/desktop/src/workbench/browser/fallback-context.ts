import {
  type RuntimeSummary, type SessionSummary, type Workspace,
} from '../../shared/contracts'
import { type Stage2WorkbenchModel } from './stage2-view-model'

export function buildFallbackContext(input: {
  readonly workspaces: readonly Workspace[]
  readonly sessions: readonly SessionSummary[]
  readonly runtimes: readonly RuntimeSummary[]
  readonly activeWorkspaceId?: string
  readonly activeSessionId?: string
}): Stage2WorkbenchModel['context'] {
  return {
    workspaces: input.workspaces.map((workspace) => ({
      id: workspace.id, name: workspace.name, detail: workspace.rootUri,
    })),
    ...(input.activeWorkspaceId !== undefined
      ? { activeWorkspaceId: input.activeWorkspaceId }
      : {}),
    sessions: input.sessions.map((session) => {
      const runtime = input.runtimes.find((candidate) => candidate.id === session.activeRuntimeId)
      return {
        id: session.id, workspaceId: session.workspaceId,
        title: session.title, state: session.status,
        ...(runtime ? { runtimeId: runtime.id, runtimeGeneration: runtime.generation } : {}),
      }
    }),
    ...(input.activeSessionId !== undefined
      ? { activeSessionId: input.activeSessionId }
      : {}),
  }
}
