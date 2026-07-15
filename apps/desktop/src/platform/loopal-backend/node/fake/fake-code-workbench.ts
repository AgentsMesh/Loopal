import { throwIfCancelled } from '../../../../base/common/cancellation'
import { type DesktopEvent } from '../../../../shared/contracts'
import { type CodeWorkbenchOperations } from '../workspace/loopal-code-workbench'
import { FakeWorkspaceService } from './fake-workspace'

export function bindFakeCodeWorkbench(
  workspaceId: string,
  emit: (event: DesktopEvent) => void,
): CodeWorkbenchOperations {
  const workspace = new FakeWorkspaceService(workspaceId, emit)
  return {
    listDirectory: workspace.listDirectory.bind(workspace),
    readFile: workspace.readFile.bind(workspace),
    writeFile: workspace.writeFile.bind(workspace),
    searchWorkspace: workspace.search.bind(workspace),
    gitStatus: workspace.gitStatus.bind(workspace),
    gitDiff: workspace.gitDiff.bind(workspace),
    gitStage: workspace.gitStage.bind(workspace),
    gitUnstage: workspace.gitUnstage.bind(workspace),
    listWorktrees: workspace.listWorktrees.bind(workspace),
    createWorktree: workspace.createWorktree.bind(workspace),
    removeWorktree: workspace.removeWorktree.bind(workspace),
    respondPermission: async (input, token) => {
      throwIfCancelled(token)
      emit({
        type: 'permission_resolved', sessionId: input.sessionId,
        runtimeId: input.runtimeId, generation: input.generation,
        agentId: input.agentId, requestId: input.requestId,
      })
    },
    respondQuestion: async (input, token) => {
      throwIfCancelled(token)
      emit({
        type: 'question_resolved', sessionId: input.sessionId,
        runtimeId: input.runtimeId, generation: input.generation,
        agentId: input.agentId, requestId: input.requestId,
      })
    },
    respondPlanApproval: async (input, token) => {
      throwIfCancelled(token)
      emit({
        type: 'plan_approval_resolved', sessionId: input.sessionId,
        runtimeId: input.runtimeId, generation: input.generation,
        agentId: input.agentId, requestId: input.requestId,
      })
    },
  }
}
