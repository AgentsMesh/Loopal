import { type CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  DirectoryListingSchema,
  FileDocumentSchema,
  GitDiffSchema,
  GitStatusSchema,
  WorkspaceSearchResultSchema,
  WorktreeListSchema,
  WorktreeSchema,
  type CreateWorktreeInput,
  type GitStageInput,
  type GitUnstageInput,
  type ListDirectoryInput,
  type PermissionResponseInput,
  type PlanApprovalResponseInput,
  type QuestionResponseInput,
  type ReadFileInput,
  type RemoveWorktreeInput,
  type WorkspaceSearchInput,
  type WriteFileInput,
} from '../../../../shared/contracts'
import { type DesktopBackend } from '../../common/backend'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'
import {
  respondPermission, respondPlanApproval, respondQuestion,
} from '../attention/loopal-code-attention'
export type CodeWorkbenchOperations = Pick<DesktopBackend,
  'listDirectory' | 'readFile' | 'writeFile' | 'searchWorkspace' |
  'gitStatus' | 'gitDiff' | 'gitStage' | 'gitUnstage' |
  'listWorktrees' | 'createWorktree' | 'removeWorktree' |
  'respondPermission' | 'respondQuestion' | 'respondPlanApproval'
>

export interface CodeWorkbenchRuntimeRouter {
  workspace(workspaceId: string): Promise<SessionRuntimeHandle>
  liveSession(sessionId: string): Promise<SessionRuntimeHandle | undefined>
}

export class LoopalCodeWorkbench implements CodeWorkbenchOperations {
  constructor(private readonly router: CodeWorkbenchRuntimeRouter) {}
  async listDirectory(input: ListDirectoryInput, token: CancellationToken) {
    return DirectoryListingSchema.parse(await this.workspaceCall(
      input.workspaceId, 'workspace/listDirectory', input, token,
    ))
  }
  async readFile(input: ReadFileInput, token: CancellationToken) {
    return FileDocumentSchema.parse(await this.workspaceCall(
      input.workspaceId, 'workspace/readFile', input, token,
    ))
  }
  async writeFile(input: WriteFileInput, token: CancellationToken) {
    return FileDocumentSchema.parse(await this.workspaceCall(
      input.workspaceId, 'workspace/writeFile', input, token,
    ))
  }
  async searchWorkspace(input: WorkspaceSearchInput, token: CancellationToken) {
    return WorkspaceSearchResultSchema.parse(await this.workspaceCall(
      input.workspaceId, 'workspace/search', input, token,
    ))
  }
  async gitStatus(workspaceId: string, token: CancellationToken) {
    return GitStatusSchema.parse(await this.workspaceCall(
      workspaceId, 'workspace/gitStatus', { workspaceId }, token,
    ))
  }
  async gitDiff(input: ReadFileInput, token: CancellationToken) {
    return GitDiffSchema.parse(await this.workspaceCall(
      input.workspaceId, 'workspace/gitDiff', input, token,
    ))
  }
  async gitStage(input: GitStageInput, token: CancellationToken) {
    await this.workspaceCall(input.workspaceId, 'workspace/gitStage', input, token)
  }
  async gitUnstage(input: GitUnstageInput, token: CancellationToken) {
    await this.workspaceCall(input.workspaceId, 'workspace/gitUnstage', input, token)
  }
  async listWorktrees(workspaceId: string, token: CancellationToken) {
    return WorktreeListSchema.parse(await this.workspaceCall(
      workspaceId, 'workspace/listWorktrees', { workspaceId }, token,
    ))
  }
  async createWorktree(input: CreateWorktreeInput, token: CancellationToken) {
    return WorktreeSchema.parse(await this.workspaceCall(
      input.workspaceId, 'workspace/createWorktree', input, token,
    ))
  }
  async removeWorktree(input: RemoveWorktreeInput, token: CancellationToken) {
    await this.workspaceCall(input.workspaceId, 'workspace/removeWorktree', input, token)
  }
  async respondPermission(input: PermissionResponseInput, token: CancellationToken) {
    await respondPermission(this.router, input, token)
  }
  async respondQuestion(input: QuestionResponseInput, token: CancellationToken) {
    await respondQuestion(this.router, input, token)
  }
  async respondPlanApproval(input: PlanApprovalResponseInput, token: CancellationToken) {
    await respondPlanApproval(this.router, input, token)
  }
  private async workspaceCall(
    workspaceId: string,
    method: string,
    params: unknown,
    token: CancellationToken,
  ): Promise<unknown> {
    const runtime = await this.resolve(() => this.router.workspace(workspaceId), token)
    return this.call(runtime, method, params, token)
  }
  private async resolve<T>(factory: () => Promise<T>, token: CancellationToken): Promise<T> {
    throwIfCancelled(token)
    const resolved = await factory()
    throwIfCancelled(token)
    return resolved
  }

  private async call(
    runtime: Pick<SessionRuntimeHandle, 'host'>,
    method: string,
    params: unknown,
    token: CancellationToken,
  ): Promise<unknown> {
    throwIfCancelled(token)
    const controller = new AbortController()
    const subscription = token.onCancellationRequested(() => controller.abort())
    try {
      const result = await runtime.host.request(method, params, controller.signal)
      throwIfCancelled(token)
      return result
    } finally {
      subscription.dispose()
    }
  }
}
