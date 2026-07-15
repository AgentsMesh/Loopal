import {
  type CodeWorkbenchOperations,
  LoopalCodeWorkbench,
} from './loopal-code-workbench'

export function bindCodeWorkbench(service: LoopalCodeWorkbench): CodeWorkbenchOperations {
  return {
    listDirectory: service.listDirectory.bind(service),
    readFile: service.readFile.bind(service),
    writeFile: service.writeFile.bind(service),
    searchWorkspace: service.searchWorkspace.bind(service),
    gitStatus: service.gitStatus.bind(service),
    gitDiff: service.gitDiff.bind(service),
    gitStage: service.gitStage.bind(service),
    gitUnstage: service.gitUnstage.bind(service),
    listWorktrees: service.listWorktrees.bind(service),
    createWorktree: service.createWorktree.bind(service),
    removeWorktree: service.removeWorktree.bind(service),
    respondPermission: service.respondPermission.bind(service),
    respondQuestion: service.respondQuestion.bind(service),
    respondPlanApproval: service.respondPlanApproval.bind(service),
  }
}
