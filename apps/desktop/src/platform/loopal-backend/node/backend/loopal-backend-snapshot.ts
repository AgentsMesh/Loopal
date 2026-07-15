import {
  type HostStatus, type RuntimeSummary, type SessionSummary,
  type WorkbenchBootstrap, type Workspace,
} from '../../../../shared/contracts'

export function backendSnapshot(input: {
  readonly hostStatus: HostStatus
  readonly workspaces: readonly Workspace[]
  readonly sessions: readonly SessionSummary[]
  readonly runtimes: readonly RuntimeSummary[]
  readonly activeSessionId?: string
}): WorkbenchBootstrap {
  return {
    protocolVersion: 2,
    hostStatus: input.hostStatus,
    workspaces: [...input.workspaces],
    sessions: [...input.sessions],
    runtimes: [...input.runtimes],
    ...(input.activeSessionId ? { activeSessionId: input.activeSessionId } : {}),
  }
}
