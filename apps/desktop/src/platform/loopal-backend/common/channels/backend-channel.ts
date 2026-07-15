import { type CancellationToken } from '../../../../base/common/cancellation'
import { type Event } from '../../../../base/common/event'
import { RemoteError } from '../../../ipc/common/wire'
import { type ServerChannel } from '../../../ipc/common/channel'
import {
  AgentControlInputSchema,
  AgentControlTargetSchema,
  CreateSessionInputSchema,
  CreateWorktreeInputSchema,
  DirectoryListingSchema,
  DesktopPreferencesSchema,
  DesktopEventSchema,
  FileDocumentSchema,
  GitDiffInputSchema,
  GitDiffSchema,
  GitStageInputSchema,
  GitStatusSchema,
  GitUnstageInputSchema,
  ListDirectoryInputSchema,
  LoopalDefaultSettingsSchema,
  LoopalSettingsWorkspaceInputSchema,
  ReadFileInputSchema,
  RemoveWorktreeInputSchema,
  SendMessageInputSchema,
  SessionDetailSchema,
  SessionOperationInputSchema,
  RuntimeSummarySchema,
  UpdateDesktopPreferencesInputSchema,
  UpdateLoopalSettingsInputSchema,
  WorkspaceOperationInputSchema,
  WorkspaceSearchInputSchema,
  WorkspaceSearchResultSchema,
  WorkbenchBootstrapSchema,
  WorktreeListSchema,
  WorktreeSchema,
  WriteFileInputSchema,
} from '../../../../shared/contracts'
import { type DesktopBackend } from '../backend'
import { callMetaHubBackend } from './metahub-channel'
import { callMcpSettingsBackend } from './mcp-settings-channel'
import { callAttentionBackend } from './attention-channel'
import { callSkillPluginBackend } from './skill-plugin-channel'

export class DesktopBackendChannel<Context> implements ServerChannel<Context> {
  constructor(private readonly backend: DesktopBackend) {}

  async call(
    _context: Context,
    command: string,
    arg: unknown,
    token: CancellationToken,
  ): Promise<unknown> {
    switch (command) {
      case 'bootstrap':
        return WorkbenchBootstrapSchema.parse(await this.backend.bootstrap(token))
      case 'openSession': {
        const { sessionId } = SessionOperationInputSchema.parse(arg)
        return SessionDetailSchema.parse(await this.backend.openSession(sessionId, token))
      }
      case 'createSession': {
        const input = CreateSessionInputSchema.parse(arg)
        return SessionDetailSchema.parse(await this.backend.createSession(input, token))
      }
      case 'stopSession': {
        const { sessionId } = SessionOperationInputSchema.parse(arg)
        await this.backend.stopSession(sessionId, token)
        return undefined
      }
      case 'restartSession': {
        const { sessionId } = SessionOperationInputSchema.parse(arg)
        return RuntimeSummarySchema.parse(await this.backend.restartSession(sessionId, token))
      }
      case 'sendMessage': {
        const { sessionId, text, agentId, images } = SendMessageInputSchema.parse(arg)
        await this.backend.sendMessage(sessionId, text, token, agentId, images)
        return undefined
      }
      case 'interruptAgent': {
        const input = AgentControlTargetSchema.parse(arg)
        await this.backend.interruptAgent(input, token)
        return undefined
      }
      case 'controlAgent': {
        const input = AgentControlInputSchema.parse(arg)
        await this.backend.controlAgent(input, token)
        return undefined
      }
      case 'getDesktopPreferences':
        return DesktopPreferencesSchema.parse(await this.backend.getDesktopPreferences(token))
      case 'updateDesktopPreferences': {
        const input = UpdateDesktopPreferencesInputSchema.parse(arg)
        return DesktopPreferencesSchema.parse(
          await this.backend.updateDesktopPreferences(input, token),
        )
      }
      case 'getLoopalSettings': {
        const { workspaceId } = LoopalSettingsWorkspaceInputSchema.parse(arg)
        return LoopalDefaultSettingsSchema.parse(
          await this.backend.getLoopalSettings(workspaceId, token),
        )
      }
      case 'updateLoopalSettings': {
        const input = UpdateLoopalSettingsInputSchema.parse(arg)
        return LoopalDefaultSettingsSchema.parse(
          await this.backend.updateLoopalSettings(input, token),
        )
      }
      case 'listDirectory': {
        const input = ListDirectoryInputSchema.parse(arg)
        return DirectoryListingSchema.parse(await this.backend.listDirectory(input, token))
      }
      case 'readFile': {
        const input = ReadFileInputSchema.parse(arg)
        return FileDocumentSchema.parse(await this.backend.readFile(input, token))
      }
      case 'writeFile': {
        const input = WriteFileInputSchema.parse(arg)
        return FileDocumentSchema.parse(await this.backend.writeFile(input, token))
      }
      case 'searchWorkspace': {
        const input = WorkspaceSearchInputSchema.parse(arg)
        return WorkspaceSearchResultSchema.parse(await this.backend.searchWorkspace(input, token))
      }
      case 'gitStatus': {
        const { workspaceId } = WorkspaceOperationInputSchema.parse(arg)
        return GitStatusSchema.parse(await this.backend.gitStatus(workspaceId, token))
      }
      case 'gitDiff': {
        const input = GitDiffInputSchema.parse(arg)
        return GitDiffSchema.parse(await this.backend.gitDiff(input, token))
      }
      case 'gitStage': {
        const input = GitStageInputSchema.parse(arg)
        await this.backend.gitStage(input, token)
        return undefined
      }
      case 'gitUnstage': {
        const input = GitUnstageInputSchema.parse(arg)
        await this.backend.gitUnstage(input, token)
        return undefined
      }
      case 'listWorktrees': {
        const { workspaceId } = WorkspaceOperationInputSchema.parse(arg)
        return WorktreeListSchema.parse(await this.backend.listWorktrees(workspaceId, token))
      }
      case 'createWorktree': {
        const input = CreateWorktreeInputSchema.parse(arg)
        return WorktreeSchema.parse(await this.backend.createWorktree(input, token))
      }
      case 'removeWorktree': {
        const input = RemoveWorktreeInputSchema.parse(arg)
        await this.backend.removeWorktree(input, token)
        return undefined
      }
      default:
        const attention = await callAttentionBackend(this.backend, command, arg, token)
        if (attention.handled) return attention.value
        const mcpSettings = await callMcpSettingsBackend(this.backend, command, arg, token)
        if (mcpSettings.handled) return mcpSettings.value
        const skillPlugins = await callSkillPluginBackend(this.backend, command, arg, token)
        if (skillPlugins.handled) return skillPlugins.value
        const metaHub = await callMetaHubBackend(this.backend, command, arg, token)
        if (metaHub.handled) return metaHub.value
        throw new RemoteError('COMMAND_NOT_FOUND', `Unknown desktop backend command: ${command}`)
    }
  }

  listen(_context: Context, event: string): Event<unknown> {
    if (event !== 'event') {
      throw new RemoteError('EVENT_NOT_FOUND', `Unknown desktop backend event: ${event}`)
    }
    return (listener) =>
      this.backend.onEvent((value) => listener(DesktopEventSchema.parse(value)))
  }
}
