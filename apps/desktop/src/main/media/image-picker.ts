import { dialog, ipcMain, type BrowserWindow, type IpcMainInvokeEvent } from 'electron'
import { toDisposable, type IDisposable } from '../../base/common/lifecycle'
import {
  DesktopImageAttachmentListSchema,
  type DesktopImageAttachment,
} from '../../shared/contracts'
import { SELECT_IMAGES_CHANNEL } from '../../shared/protocol/renderer-protocol'
import { loadSelectedImages } from './image-loader'

type ImageLoader = (paths: readonly string[]) => Promise<DesktopImageAttachment[]>

export function registerImagePicker(
  window: BrowserWindow,
  load: ImageLoader = loadSelectedImages,
): IDisposable {
  const handler = async (event: IpcMainInvokeEvent, ...args: unknown[]): Promise<unknown> => {
    if (event.sender !== window.webContents || event.senderFrame !== window.webContents.mainFrame) {
      throw new Error('Image selection is restricted to the main window')
    }
    if (args.length !== 0) throw new Error('Image selection does not accept file paths')
    const result = await dialog.showOpenDialog(window, {
      title: 'Attach images',
      properties: ['openFile', 'multiSelections'],
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp'] }],
    })
    if (result.canceled) return []
    return DesktopImageAttachmentListSchema.parse(await load(result.filePaths))
  }
  ipcMain.handle(SELECT_IMAGES_CHANNEL, handler)
  return toDisposable(() => ipcMain.removeHandler(SELECT_IMAGES_CHANNEL))
}
