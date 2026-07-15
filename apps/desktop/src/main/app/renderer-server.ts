import { ipcMain, MessageChannelMain, type IpcMainEvent, type WebContents } from 'electron'
import { DisposableStore, type IDisposable, toDisposable } from '../../base/common/lifecycle'
import { ChannelServer } from '../../platform/ipc/common/channel'
import { MessagePortTransport } from '../../platform/ipc/common/transport'
import { DesktopBackendChannel } from '../../platform/loopal-backend/common/channels/backend-channel'
import { type DesktopBackend } from '../../platform/loopal-backend/common/backend'
import {
  RENDERER_CONNECT_CHANNEL,
  RENDERER_PORT_CHANNEL,
  RENDERER_PROTOCOL_VERSION,
} from '../../shared/protocol/renderer-protocol'

export interface RendererContext {
  readonly webContentsId: number
  readonly frameUrl: string
  readonly principal: 'main-window'
}

export interface RendererConnectionPolicy {
  isAllowed(event: IpcMainEvent): boolean
}

export function mainWindowPolicy(webContents: WebContents): RendererConnectionPolicy {
  return {
    isAllowed: (event) =>
      event.sender === webContents && event.senderFrame === webContents.mainFrame,
  }
}

export function isSafeExternalUrl(value: string): boolean {
  try { return new URL(value).protocol === 'https:' }
  catch { return false }
}

export function registerRendererServer(
  backend: DesktopBackend,
  policy: RendererConnectionPolicy,
): IDisposable {
  const store = new DisposableStore()
  const servers = new Set<ChannelServer<RendererContext>>()

  const listener = (event: IpcMainEvent): void => {
    const frame = event.senderFrame
    if (!frame || !policy.isAllowed(event)) {
      return
    }
    const { port1, port2 } = new MessageChannelMain()
    const context: RendererContext = {
      webContentsId: event.sender.id,
      frameUrl: frame.url,
      principal: 'main-window',
    }
    const server = new ChannelServer(new MessagePortTransport(port1), context)
    server.registerChannel('desktopBackend', new DesktopBackendChannel(backend))
    servers.add(server)
    port1.once('close', () => {
      servers.delete(server)
      server.dispose()
    })
    frame.postMessage(
      RENDERER_PORT_CHANNEL,
      { protocolVersion: RENDERER_PROTOCOL_VERSION },
      [port2],
    )
  }

  ipcMain.on(RENDERER_CONNECT_CHANNEL, listener)
  store.add(toDisposable(() => ipcMain.removeListener(RENDERER_CONNECT_CHANNEL, listener)))
  store.add(
    toDisposable(() => {
      for (const server of servers) {
        server.dispose()
      }
      servers.clear()
    }),
  )
  return store
}
