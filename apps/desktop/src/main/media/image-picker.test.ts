import { beforeEach, describe, expect, it, vi } from 'vitest'
import { SELECT_IMAGES_CHANNEL } from '../../shared/protocol/renderer-protocol'

const electron = vi.hoisted(() => {
  const state: {
    handler: ((...args: unknown[]) => Promise<unknown>) | undefined
  } = { handler: undefined }
  return {
    state,
    dialog: { showOpenDialog: vi.fn() },
    ipcMain: {
      handle: vi.fn((_channel: string, handler: (...args: unknown[]) => Promise<unknown>) => {
        state.handler = handler
      }),
      removeHandler: vi.fn(),
    },
  }
})

vi.mock('electron', () => ({
  dialog: electron.dialog,
  ipcMain: electron.ipcMain,
}))

import { registerImagePicker } from './image-picker'

describe('main-process image picker', () => {
  beforeEach(() => {
    electron.state.handler = undefined
    electron.dialog.showOpenDialog.mockReset()
    electron.ipcMain.handle.mockClear()
    electron.ipcMain.removeHandler.mockClear()
  })

  it('accepts only owner-frame selections and never accepts a renderer path', async () => {
    const mainFrame = {}
    const webContents = { mainFrame }
    const window = { webContents }
    const image = {
      name: 'pixel.png', mediaType: 'image/png' as const, data: 'iVBORw==', sizeBytes: 4,
    }
    electron.dialog.showOpenDialog.mockResolvedValue({
      canceled: false, filePaths: ['/chosen/pixel.png'],
    })
    const load = vi.fn(async () => [image])
    const registration = registerImagePicker(window as never, load)
    expect(electron.ipcMain.handle).toHaveBeenCalledWith(
      SELECT_IMAGES_CHANNEL, expect.any(Function),
    )

    await expect(electron.state.handler!({ sender: {}, senderFrame: mainFrame }))
      .rejects.toThrow('restricted to the main window')
    await expect(electron.state.handler!(
      { sender: webContents, senderFrame: mainFrame }, '/renderer/path.png',
    )).rejects.toThrow('does not accept file paths')
    await expect(electron.state.handler!({ sender: webContents, senderFrame: mainFrame }))
      .resolves.toEqual([image])
    expect(load).toHaveBeenCalledWith(['/chosen/pixel.png'])

    registration.dispose()
    expect(electron.ipcMain.removeHandler).toHaveBeenCalledWith(SELECT_IMAGES_CHANNEL)
  })

  it('returns no data when the native picker is cancelled', async () => {
    const mainFrame = {}
    const webContents = { mainFrame }
    electron.dialog.showOpenDialog.mockResolvedValue({ canceled: true, filePaths: [] })
    const load = vi.fn()
    registerImagePicker({ webContents } as never, load)
    await expect(electron.state.handler!({ sender: webContents, senderFrame: mainFrame }))
      .resolves.toEqual([])
    expect(load).not.toHaveBeenCalled()
  })
})
