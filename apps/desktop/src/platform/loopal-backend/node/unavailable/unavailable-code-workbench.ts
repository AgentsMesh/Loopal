import { type CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import { type CodeWorkbenchOperations } from '../workspace/loopal-code-workbench'

export function bindUnavailableCodeWorkbench(reason: string): CodeWorkbenchOperations {
  const unavailable = async (...args: unknown[]): Promise<never> => {
    throwIfCancelled(args.at(-1) as CancellationToken)
    throw new Error(reason)
  }
  return {
    listDirectory: unavailable,
    readFile: unavailable,
    writeFile: unavailable,
    searchWorkspace: unavailable,
    gitStatus: unavailable,
    gitDiff: unavailable,
    gitStage: unavailable,
    gitUnstage: unavailable,
    listWorktrees: unavailable,
    createWorktree: unavailable,
    removeWorktree: unavailable,
    respondPermission: unavailable,
    respondQuestion: unavailable,
    respondPlanApproval: unavailable,
  }
}
