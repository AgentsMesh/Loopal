import { type HostStatus } from '../../../../shared/contracts'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

export class LoopalWorkspaceLeaders {
  private readonly members = new Map<string, Map<string, SessionRuntimeHandle>>()
  private readonly runtimeWorkspaces = new Map<string, string>()
  private readonly retiring = new Map<string, string>()

  add(runtime: SessionRuntimeHandle): boolean {
    const becameLeader = !this.current(runtime.workspaceId)
    let workspace = this.members.get(runtime.workspaceId)
    if (!workspace) {
      workspace = new Map()
      this.members.set(runtime.workspaceId, workspace)
    }
    workspace.set(runtime.runtimeId, runtime)
    this.runtimeWorkspaces.set(runtime.runtimeId, runtime.workspaceId)
    if (becameLeader) this.retiring.delete(runtime.workspaceId)
    return becameLeader
  }

  current(workspaceId: string): SessionRuntimeHandle | undefined {
    return this.members.get(workspaceId)?.values().next().value
  }

  isLeader(runtimeId: string, workspaceId: string): boolean {
    return this.current(workspaceId)?.runtimeId === runtimeId
  }

  transition(runtimeId: string, workspaceId: string, status: HostStatus): readonly HostStatus[] {
    if (status === 'stopping') {
      const transition = this.stopping(runtimeId, workspaceId)
      return transition.wasLeader
        ? [status, ...(transition.next ? [transition.next.host.currentStatus] : [])]
        : []
    }
    if (status === 'stopped' || status === 'crashed') {
      const transition = this.finished(runtimeId, workspaceId)
      return transition.publish ? [transition.next?.host.currentStatus ?? status] : []
    }
    return this.isLeader(runtimeId, workspaceId) ? [status] : []
  }

  private stopping(runtimeId: string, workspaceId: string) {
    const wasLeader = this.isLeader(runtimeId, workspaceId)
    const next = this.remove(runtimeId)
    if (wasLeader && !next) this.retiring.set(workspaceId, runtimeId)
    return { wasLeader, next }
  }

  private finished(runtimeId: string, workspaceId: string) {
    const wasLeader = this.isLeader(runtimeId, workspaceId)
    const next = this.remove(runtimeId)
    const wasRetiring = this.retiring.get(workspaceId) === runtimeId
    if (wasRetiring) this.retiring.delete(workspaceId)
    return { publish: wasLeader || wasRetiring, next }
  }

  private remove(runtimeId: string): SessionRuntimeHandle | undefined {
    const workspaceId = this.runtimeWorkspaces.get(runtimeId)
    if (!workspaceId) return undefined
    this.runtimeWorkspaces.delete(runtimeId)
    const workspace = this.members.get(workspaceId)
    workspace?.delete(runtimeId)
    if (workspace?.size === 0) this.members.delete(workspaceId)
    return this.current(workspaceId)
  }
}
