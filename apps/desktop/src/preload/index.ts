import { contextBridge, ipcRenderer } from 'electron'
import { ChannelClientImpl } from '../platform/ipc/common/channel'
import { MessagePortTransport } from '../platform/ipc/common/transport'
import { DesktopBackendClient } from '../platform/loopal-backend/common/clients/backend-client'
import {
  RENDERER_CONNECT_CHANNEL,
  RENDERER_PORT_CHANNEL,
  RENDERER_PROTOCOL_VERSION,
  SELECT_IMAGES_CHANNEL,
  SELECT_SESSION_DIRECTORY_CHANNEL,
} from '../shared/protocol/renderer-protocol'
import {
  DesktopImageAttachmentListSchema,
  type DesktopEvent,
  type LoopalDesktopAPI,
  SessionDirectorySelectionSchema,
} from '../shared/contracts'

let apiPromise: Promise<DesktopBackendClient> | undefined

function connect(): Promise<DesktopBackendClient> {
  apiPromise ??= new Promise((resolve, reject) => {
    ipcRenderer.once(RENDERER_PORT_CHANNEL, (event, metadata: unknown) => {
      if (
        typeof metadata !== 'object' ||
        metadata === null ||
        !('protocolVersion' in metadata) ||
        metadata.protocolVersion !== RENDERER_PROTOCOL_VERSION ||
        !event.ports[0]
      ) {
        reject(new Error('Invalid LoopalDesktop renderer connection'))
        return
      }
      const client = new ChannelClientImpl(new MessagePortTransport(event.ports[0]))
      resolve(new DesktopBackendClient(client, async () => DesktopImageAttachmentListSchema.parse(
        await ipcRenderer.invoke(SELECT_IMAGES_CHANNEL),
      )))
    })
    ipcRenderer.send(RENDERER_CONNECT_CHANNEL)
  })
  return apiPromise
}

const api: LoopalDesktopAPI = {
  bootstrap: async () => (await connect()).bootstrap(),
  openSession: async (sessionId) => (await connect()).openSession(sessionId),
  createSession: async (input) => (await connect()).createSession(input),
  selectSessionDirectory: async () => SessionDirectorySelectionSchema.optional().parse(
    await ipcRenderer.invoke(SELECT_SESSION_DIRECTORY_CHANNEL),
  ),
  stopSession: async (sessionId) => (await connect()).stopSession(sessionId),
  restartSession: async (sessionId) => (await connect()).restartSession(sessionId),
  selectImages: async () => (await connect()).selectImages(),
  sendMessage: async (sessionId, text, agentId, images) => (
    await connect()
  ).sendMessage(sessionId, text, agentId, images),
  interruptAgent: async (input) => (await connect()).interruptAgent(input),
  controlAgent: async (input) => (await connect()).controlAgent(input),
  getDesktopPreferences: async () => (await connect()).getDesktopPreferences(),
  updateDesktopPreferences: async (input) => (await connect()).updateDesktopPreferences(input),
  getLoopalSettings: async (workspaceId) => (await connect()).getLoopalSettings(workspaceId),
  updateLoopalSettings: async (input) => (await connect()).updateLoopalSettings(input),
  listMcpServers: async (workspaceId) => (await connect()).listMcpServers(workspaceId),
  upsertMcpServer: async (input) => (await connect()).upsertMcpServer(input),
  deleteMcpServer: async (input) => (await connect()).deleteMcpServer(input),
  listSkills: async (workspaceId) => (await connect()).listSkills(workspaceId),
  getSkill: async (input) => (await connect()).getSkill(input),
  upsertGlobalSkill: async (input) => (await connect()).upsertGlobalSkill(input),
  deleteGlobalSkill: async (input) => (await connect()).deleteGlobalSkill(input),
  listPlugins: async (workspaceId) => (await connect()).listPlugins(workspaceId),
  getMetaHubSettings: async () => (await connect()).getMetaHubSettings(),
  updateMetaHubSettings: async (input) => (await connect()).updateMetaHubSettings(input),
  getMetaHubStatus: async (target) => (await connect()).getMetaHubStatus(target),
  joinMetaHub: async (input) => (await connect()).joinMetaHub(input),
  disconnectMetaHub: async (target) => (await connect()).disconnectMetaHub(target),
  getLocalMetaHubStatus: async () => (await connect()).getLocalMetaHubStatus(),
  startLocalMetaHub: async (input) => (await connect()).startLocalMetaHub(input),
  stopLocalMetaHub: async () => (await connect()).stopLocalMetaHub(),
  listDirectory: async (input) => (await connect()).listDirectory(input),
  readFile: async (input) => (await connect()).readFile(input),
  writeFile: async (input) => (await connect()).writeFile(input),
  searchWorkspace: async (input) => (await connect()).searchWorkspace(input),
  gitStatus: async (workspaceId) => (await connect()).gitStatus(workspaceId),
  gitDiff: async (input) => (await connect()).gitDiff(input),
  gitStage: async (input) => (await connect()).gitStage(input),
  gitUnstage: async (input) => (await connect()).gitUnstage(input),
  listWorktrees: async (workspaceId) => (await connect()).listWorktrees(workspaceId),
  createWorktree: async (input) => (await connect()).createWorktree(input),
  removeWorktree: async (input) => (await connect()).removeWorktree(input),
  respondPermission: async (input) => (await connect()).respondPermission(input),
  respondQuestion: async (input) => (await connect()).respondQuestion(input),
  respondPlanApproval: async (input) => (await connect()).respondPlanApproval(input),
  onEvent: (listener: (event: DesktopEvent) => void) => {
    let unsubscribe: (() => void) | undefined
    let disposed = false
    void connect().then((client) => {
      if (!disposed) {
        unsubscribe = client.onEvent(listener)
      }
    })
    return () => {
      disposed = true
      unsubscribe?.()
    }
  },
}

contextBridge.exposeInMainWorld('loopalDesktop', api)
