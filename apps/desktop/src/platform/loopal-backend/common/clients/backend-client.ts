import { type Event } from '../../../../base/common/event'
import { type ChannelClient } from '../../../ipc/common/channel'
import {
  DirectoryListingSchema,
  DesktopEventSchema,
  FileDocumentSchema,
  GitDiffSchema,
  GitStatusSchema,
  LoopalDefaultSettingsSchema,
  RuntimeSummarySchema,
  SessionDetailSchema,
  WorkspaceSearchResultSchema,
  WorkbenchBootstrapSchema,
  WorktreeListSchema,
  WorktreeSchema,
  type AgentControlInput,
  type AgentControlTarget,
  type CreateSessionInput,
  type CreateWorktreeInput,
  type DirectoryListing,
  type DesktopImageAttachment,
  type DesktopEvent,
  type FileDocument,
  type GitDiff,
  type GitStageInput,
  type GitStatus,
  type GitUnstageInput,
  type ListDirectoryInput,
  type LoopalDesktopAPI,
  type LoopalDefaultSettings,
  type ReadFileInput,
  type RemoveWorktreeInput,
  type RuntimeSummary,
  type SessionDirectorySelection,
  type SessionDetail,
  type UpdateLoopalSettingsInput,
  type WorkspaceSearchInput,
  type WorkspaceSearchResult,
  type WorkbenchBootstrap,
  type Worktree,
  type WriteFileInput,
} from '../../../../shared/contracts'
import { bindMetaHubClient, type MetaHubClientOperations } from './metahub-client'
import { bindMcpSettingsClient, type McpSettingsClientOperations } from './mcp-settings-client'
import { bindSkillPluginClient, type SkillPluginClientOperations } from './skill-plugin-client'
import { bindAttentionClient, type AttentionClientOperations } from './attention-client'
import {
  bindDesktopPreferencesClient,
  type DesktopPreferencesClientOperations,
} from './desktop-preferences-client'

export interface DesktopBackendClient extends
  MetaHubClientOperations, McpSettingsClientOperations, AttentionClientOperations,
  DesktopPreferencesClientOperations, SkillPluginClientOperations {}
export class DesktopBackendClient implements LoopalDesktopAPI {
  constructor(
    private readonly client: ChannelClient,
    private readonly imageSelector: () => Promise<DesktopImageAttachment[]> = async () => [],
    private readonly directorySelector: () => Promise<
      SessionDirectorySelection | undefined
    > = async () => undefined,
  ) {
    Object.assign(this, bindMetaHubClient(client))
    Object.assign(this, bindMcpSettingsClient(client))
    Object.assign(this, bindSkillPluginClient(client))
    Object.assign(this, bindAttentionClient(client))
    Object.assign(this, bindDesktopPreferencesClient(client))
  }

  async bootstrap(): Promise<WorkbenchBootstrap> {
    return WorkbenchBootstrapSchema.parse(
      await this.client.call('desktopBackend', 'bootstrap'),
    )
  }

  async openSession(sessionId: string): Promise<SessionDetail> {
    return SessionDetailSchema.parse(
      await this.client.call('desktopBackend', 'openSession', { sessionId }),
    )
  }

  async createSession(input: CreateSessionInput): Promise<SessionDetail> {
    return SessionDetailSchema.parse(
      await this.client.call('desktopBackend', 'createSession', input),
    )
  }

  async stopSession(sessionId: string): Promise<void> {
    await this.client.call('desktopBackend', 'stopSession', { sessionId })
  }
  async restartSession(sessionId: string): Promise<RuntimeSummary> {
    return RuntimeSummarySchema.parse(
      await this.client.call('desktopBackend', 'restartSession', { sessionId }),
    )
  }

  selectImages(): Promise<DesktopImageAttachment[]> { return this.imageSelector() }
  selectSessionDirectory(): Promise<SessionDirectorySelection | undefined> {
    return this.directorySelector()
  }

  async sendMessage(
    sessionId: string, text: string, agentId?: string,
    images?: readonly DesktopImageAttachment[],
  ): Promise<void> {
    await this.client.call('desktopBackend', 'sendMessage', {
      sessionId, text, ...(agentId ? { agentId } : {}), ...(images?.length ? { images } : {}),
    })
  }

  async interruptAgent(input: AgentControlTarget): Promise<void> {
    await this.client.call('desktopBackend', 'interruptAgent', input)
  }
  async controlAgent(input: AgentControlInput): Promise<void> {
    await this.client.call('desktopBackend', 'controlAgent', input)
  }

  async getLoopalSettings(workspaceId: string): Promise<LoopalDefaultSettings> {
    return LoopalDefaultSettingsSchema.parse(
      await this.client.call('desktopBackend', 'getLoopalSettings', { workspaceId }),
    )
  }

  async updateLoopalSettings(input: UpdateLoopalSettingsInput): Promise<LoopalDefaultSettings> {
    return LoopalDefaultSettingsSchema.parse(
      await this.client.call('desktopBackend', 'updateLoopalSettings', input),
    )
  }

  async listDirectory(input: ListDirectoryInput): Promise<DirectoryListing> {
    return DirectoryListingSchema.parse(
      await this.client.call('desktopBackend', 'listDirectory', input),
    )
  }

  async readFile(input: ReadFileInput): Promise<FileDocument> {
    return FileDocumentSchema.parse(await this.client.call('desktopBackend', 'readFile', input))
  }
  async writeFile(input: WriteFileInput): Promise<FileDocument> {
    return FileDocumentSchema.parse(await this.client.call('desktopBackend', 'writeFile', input))
  }

  async searchWorkspace(input: WorkspaceSearchInput): Promise<WorkspaceSearchResult> {
    return WorkspaceSearchResultSchema.parse(
      await this.client.call('desktopBackend', 'searchWorkspace', input),
    )
  }

  async gitStatus(workspaceId: string): Promise<GitStatus> {
    return GitStatusSchema.parse(
      await this.client.call('desktopBackend', 'gitStatus', { workspaceId }),
    )
  }

  async gitDiff(input: ReadFileInput): Promise<GitDiff> {
    return GitDiffSchema.parse(await this.client.call('desktopBackend', 'gitDiff', input))
  }
  async gitStage(input: GitStageInput): Promise<void> {
    await this.client.call('desktopBackend', 'gitStage', input)
  }

  async gitUnstage(input: GitUnstageInput): Promise<void> {
    await this.client.call('desktopBackend', 'gitUnstage', input)
  }

  async listWorktrees(workspaceId: string): Promise<readonly Worktree[]> {
    return WorktreeListSchema.parse(
      await this.client.call('desktopBackend', 'listWorktrees', { workspaceId }),
    )
  }

  async createWorktree(input: CreateWorktreeInput): Promise<Worktree> {
    return WorktreeSchema.parse(
      await this.client.call('desktopBackend', 'createWorktree', input),
    )
  }

  async removeWorktree(input: RemoveWorktreeInput): Promise<void> {
    await this.client.call('desktopBackend', 'removeWorktree', input)
  }

  onEvent(listener: (event: DesktopEvent) => void): () => void {
    const event: Event<unknown> = this.client.listen('desktopBackend', 'event')
    const subscription = event((value) => listener(DesktopEventSchema.parse(value)))
    return () => subscription.dispose()
  }
}
