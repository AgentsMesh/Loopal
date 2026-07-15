import { type Event } from '../../../base/common/event'
import { createServiceIdentifier } from '../../instantiation/common/services'
import {
  type AgentControlInput,
  type AgentControlTarget,
  type CreateSessionInput,
  type CreateWorktreeInput,
  type DirectoryListing,
  type DesktopEvent,
  type DesktopImageAttachment,
  type DesktopPreferences,
  type FileDocument,
  type GitDiff,
  type GitStageInput,
  type GitStatus,
  type GitUnstageInput,
  type ListDirectoryInput,
  type JoinMetaHubInput,
  type LocalMetaHubStatus,
  type LoopalDefaultSettings,
  type DeleteGlobalSkillInput,
  type DeleteMcpServerInput,
  type GetSkillInput,
  type McpServersResponse,
  type PluginsResponse,
  type MetaHubRuntimeState,
  type MetaHubRuntimeTarget,
  type MetaHubSettings,
  type PermissionResponseInput,
  type PlanApprovalResponseInput,
  type QuestionResponseInput,
  type ReadFileInput,
  type RemoveWorktreeInput,
  type RuntimeSummary,
  type SessionDetail,
  type SkillDetail,
  type SkillsResponse,
  type StartLocalMetaHubInput,
  type UpdateMetaHubSettingsInput,
  type UpdateDesktopPreferencesInput,
  type UpdateLoopalSettingsInput,
  type UpsertGlobalSkillInput,
  type UpsertMcpServerInput,
  type WorkspaceSearchInput,
  type WorkspaceSearchResult,
  type WorkbenchBootstrap,
  type Worktree,
  type WriteFileInput,
} from '../../../shared/contracts'
import { type CancellationToken } from '../../../base/common/cancellation'

export interface DesktopBackend {
  readonly onEvent: Event<DesktopEvent>
  bootstrap(token: CancellationToken): Promise<WorkbenchBootstrap>
  openSession(sessionId: string, token: CancellationToken): Promise<SessionDetail>
  createSession(input: CreateSessionInput, token: CancellationToken): Promise<SessionDetail>
  stopSession(sessionId: string, token: CancellationToken): Promise<void>
  restartSession(sessionId: string, token: CancellationToken): Promise<RuntimeSummary>
  sendMessage(
    sessionId: string, text: string, token: CancellationToken, agentId?: string,
    images?: readonly DesktopImageAttachment[],
  ): Promise<void>
  interruptAgent(input: AgentControlTarget, token: CancellationToken): Promise<void>
  controlAgent(input: AgentControlInput, token: CancellationToken): Promise<void>
  getDesktopPreferences(token: CancellationToken): Promise<DesktopPreferences>
  updateDesktopPreferences(
    input: UpdateDesktopPreferencesInput, token: CancellationToken,
  ): Promise<DesktopPreferences>
  getLoopalSettings(
    workspaceId: string, token: CancellationToken,
  ): Promise<LoopalDefaultSettings>
  updateLoopalSettings(
    input: UpdateLoopalSettingsInput, token: CancellationToken,
  ): Promise<LoopalDefaultSettings>
  listMcpServers(workspaceId: string, token: CancellationToken): Promise<McpServersResponse>
  upsertMcpServer(
    input: UpsertMcpServerInput, token: CancellationToken,
  ): Promise<McpServersResponse>
  deleteMcpServer(
    input: DeleteMcpServerInput, token: CancellationToken,
  ): Promise<McpServersResponse>
  listSkills(workspaceId: string, token: CancellationToken): Promise<SkillsResponse>
  getSkill(input: GetSkillInput, token: CancellationToken): Promise<SkillDetail>
  upsertGlobalSkill(
    input: UpsertGlobalSkillInput, token: CancellationToken,
  ): Promise<SkillDetail>
  deleteGlobalSkill(
    input: DeleteGlobalSkillInput, token: CancellationToken,
  ): Promise<SkillsResponse>
  listPlugins(workspaceId: string, token: CancellationToken): Promise<PluginsResponse>
  getMetaHubSettings(token: CancellationToken): Promise<MetaHubSettings>
  updateMetaHubSettings(
    input: UpdateMetaHubSettingsInput, token: CancellationToken,
  ): Promise<MetaHubSettings>
  getMetaHubStatus(
    target: MetaHubRuntimeTarget, token: CancellationToken,
  ): Promise<MetaHubRuntimeState>
  joinMetaHub(input: JoinMetaHubInput, token: CancellationToken): Promise<MetaHubRuntimeState>
  disconnectMetaHub(
    target: MetaHubRuntimeTarget, token: CancellationToken,
  ): Promise<MetaHubRuntimeState>
  getLocalMetaHubStatus(token: CancellationToken): Promise<LocalMetaHubStatus>
  startLocalMetaHub(
    input: StartLocalMetaHubInput, token: CancellationToken,
  ): Promise<LocalMetaHubStatus>
  stopLocalMetaHub(token: CancellationToken): Promise<LocalMetaHubStatus>
  listDirectory(input: ListDirectoryInput, token: CancellationToken): Promise<DirectoryListing>
  readFile(input: ReadFileInput, token: CancellationToken): Promise<FileDocument>
  writeFile(input: WriteFileInput, token: CancellationToken): Promise<FileDocument>
  searchWorkspace(
    input: WorkspaceSearchInput,
    token: CancellationToken,
  ): Promise<WorkspaceSearchResult>
  gitStatus(workspaceId: string, token: CancellationToken): Promise<GitStatus>
  gitDiff(input: ReadFileInput, token: CancellationToken): Promise<GitDiff>
  gitStage(input: GitStageInput, token: CancellationToken): Promise<void>
  gitUnstage(input: GitUnstageInput, token: CancellationToken): Promise<void>
  listWorktrees(workspaceId: string, token: CancellationToken): Promise<readonly Worktree[]>
  createWorktree(input: CreateWorktreeInput, token: CancellationToken): Promise<Worktree>
  removeWorktree(input: RemoveWorktreeInput, token: CancellationToken): Promise<void>
  respondPermission(input: PermissionResponseInput, token: CancellationToken): Promise<void>
  respondQuestion(input: QuestionResponseInput, token: CancellationToken): Promise<void>
  respondPlanApproval(input: PlanApprovalResponseInput, token: CancellationToken): Promise<void>
}

export const IDesktopBackend = createServiceIdentifier<DesktopBackend>('desktopBackend')
