import { z } from 'zod'
import {
  type AgentControlDisposition,
  type AgentControlInput,
  type AgentControlTarget,
} from './control-contracts'
import {
  AttentionDesktopEventSchema,
  type PermissionResponseInput,
  type PlanApprovalResponseInput,
  type QuestionResponseInput,
} from './attention-contracts'
import {
  type CreateSessionInput,
  RuntimeSummarySchema,
  SessionDetailSchema,
  SessionSummarySchema,
  type RuntimeSummary,
  type SessionDetail,
  type SessionDirectorySelection,
  ConversationEntrySchema,
  ArtifactSchema,
} from './session-contracts'
import {
  type CreateWorktreeInput, type DirectoryListing, type FileDocument, type GitDiff,
  type GitStageInput, type GitStatus, type GitUnstageInput, type ListDirectoryInput,
  type ReadFileInput, type RemoveWorktreeInput, type WorkspaceSearchInput,
  type WorkspaceSearchResult, type Worktree, type WriteFileInput,
  WorkspaceDesktopEventSchema,
} from './workspace-contracts'
import {
  type JoinMetaHubInput,
  type LocalMetaHubStatus,
  type MetaHubRuntimeState,
  type MetaHubRuntimeTarget,
  type MetaHubSettings,
  type StartLocalMetaHubInput,
  type UpdateMetaHubSettingsInput,
} from './metahub-contracts'
import {
  type LoopalDefaultSettings,
  type UpdateLoopalSettingsInput,
} from './loopal-settings-contracts'
import {
  type DeleteMcpServerInput,
  type McpServersResponse,
  type UpsertMcpServerInput,
} from './mcp-settings-contracts'
import {
  type DeleteGlobalSkillInput,
  type GetSkillInput,
  type PluginsResponse,
  type SkillDetail,
  type SkillsResponse,
  type UpsertGlobalSkillInput,
} from './skill-plugin-contracts'
import { type DesktopImageAttachment } from './image-contracts'
import {
  type DesktopPreferences,
  type UpdateDesktopPreferencesInput,
} from './desktop-preferences-contracts'

export * from './attention-contracts'
export * from './control-contracts'
export * from './desktop-preferences-contracts'
export * from './image-contracts'
export * from './metahub-contracts'
export * from './loopal-settings-contracts'
export * from './mcp-settings-contracts'
export * from './skill-plugin-contracts'
export * from './session-contracts'
export * from './workspace-contracts'

export const HostStatusSchema = z.enum([
  'stopped', 'spawning', 'alive', 'registering', 'ready', 'stopping', 'crashed',
])
export type HostStatus = z.infer<typeof HostStatusSchema>

export const WorkspaceSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  rootUri: z.string().min(1),
  kind: z.enum(['folder', 'git_worktree', 'remote', 'cloud']),
})
export type Workspace = z.infer<typeof WorkspaceSchema>

export const WorkbenchBootstrapSchema = z.object({
  protocolVersion: z.literal(2),
  hostStatus: HostStatusSchema,
  workspaces: z.array(WorkspaceSchema),
  sessions: z.array(SessionSummarySchema),
  runtimes: z.array(RuntimeSummarySchema),
  activeSessionId: z.string().optional(),
})
export type WorkbenchBootstrap = z.infer<typeof WorkbenchBootstrapSchema>

const CoreDesktopEventSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('host_status'), status: HostStatusSchema }),
  z.object({ type: z.literal('session_updated'), session: SessionSummarySchema }),
  z.object({ type: z.literal('runtime_updated'), runtime: RuntimeSummarySchema }),
  z.object({ type: z.literal('session_detail_replaced'), detail: SessionDetailSchema }),
  z.object({
    type: z.literal('conversation_entry'),
    sessionId: z.string().min(1),
    entry: ConversationEntrySchema,
  }),
  z.object({ type: z.literal('artifact_created'), artifact: ArtifactSchema }),
])
export const DesktopEventSchema = z.union([
  CoreDesktopEventSchema,
  WorkspaceDesktopEventSchema,
  AttentionDesktopEventSchema,
])
export type DesktopEvent = z.infer<typeof DesktopEventSchema>

export interface LoopalDesktopAPI {
  bootstrap(): Promise<WorkbenchBootstrap>
  openSession(sessionId: string): Promise<SessionDetail>
  createSession(input: CreateSessionInput): Promise<SessionDetail>
  selectSessionDirectory(): Promise<SessionDirectorySelection | undefined>
  stopSession(sessionId: string): Promise<void>
  restartSession(sessionId: string): Promise<RuntimeSummary>
  selectImages(): Promise<DesktopImageAttachment[]>
  sendMessage(
    sessionId: string, text: string, agentId?: string,
    images?: readonly DesktopImageAttachment[],
  ): Promise<void>
  interruptAgent(target: AgentControlTarget): Promise<void>
  controlAgent(input: AgentControlInput): Promise<AgentControlDisposition>
  getDesktopPreferences(): Promise<DesktopPreferences>
  updateDesktopPreferences(input: UpdateDesktopPreferencesInput): Promise<DesktopPreferences>
  getLoopalSettings(workspaceId: string): Promise<LoopalDefaultSettings>
  updateLoopalSettings(input: UpdateLoopalSettingsInput): Promise<LoopalDefaultSettings>
  listMcpServers(workspaceId: string): Promise<McpServersResponse>
  upsertMcpServer(input: UpsertMcpServerInput): Promise<McpServersResponse>
  deleteMcpServer(input: DeleteMcpServerInput): Promise<McpServersResponse>
  listSkills(workspaceId: string): Promise<SkillsResponse>
  getSkill(input: GetSkillInput): Promise<SkillDetail>
  upsertGlobalSkill(input: UpsertGlobalSkillInput): Promise<SkillDetail>
  deleteGlobalSkill(input: DeleteGlobalSkillInput): Promise<SkillsResponse>
  listPlugins(workspaceId: string): Promise<PluginsResponse>
  getMetaHubSettings(): Promise<MetaHubSettings>
  updateMetaHubSettings(input: UpdateMetaHubSettingsInput): Promise<MetaHubSettings>
  getMetaHubStatus(target: MetaHubRuntimeTarget): Promise<MetaHubRuntimeState>
  joinMetaHub(input: JoinMetaHubInput): Promise<MetaHubRuntimeState>
  disconnectMetaHub(target: MetaHubRuntimeTarget): Promise<MetaHubRuntimeState>
  getLocalMetaHubStatus(): Promise<LocalMetaHubStatus>
  startLocalMetaHub(input: StartLocalMetaHubInput): Promise<LocalMetaHubStatus>
  stopLocalMetaHub(): Promise<LocalMetaHubStatus>
  listDirectory(input: ListDirectoryInput): Promise<DirectoryListing>
  readFile(input: ReadFileInput): Promise<FileDocument>
  writeFile(input: WriteFileInput): Promise<FileDocument>
  searchWorkspace(input: WorkspaceSearchInput): Promise<WorkspaceSearchResult>
  gitStatus(workspaceId: string): Promise<GitStatus>
  gitDiff(input: ReadFileInput): Promise<GitDiff>
  gitStage(input: GitStageInput): Promise<void>
  gitUnstage(input: GitUnstageInput): Promise<void>
  listWorktrees(workspaceId: string): Promise<readonly Worktree[]>
  createWorktree(input: CreateWorktreeInput): Promise<Worktree>
  removeWorktree(input: RemoveWorktreeInput): Promise<void>
  respondPermission(input: PermissionResponseInput): Promise<void>
  respondQuestion(input: QuestionResponseInput): Promise<void>
  respondPlanApproval(input: PlanApprovalResponseInput): Promise<void>
  onEvent(listener: (event: DesktopEvent) => void): () => void
}
