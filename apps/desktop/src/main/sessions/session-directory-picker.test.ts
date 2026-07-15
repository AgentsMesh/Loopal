import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SELECT_SESSION_DIRECTORY_CHANNEL } from '../../shared/protocol/renderer-protocol'

const electron = vi.hoisted(() => {
  const state: { handler?: (...args: unknown[]) => Promise<unknown> } = {}
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
vi.mock('electron', () => ({ dialog: electron.dialog, ipcMain: electron.ipcMain }))

import { registerSessionDirectoryPicker } from './session-directory-picker'

const selection = {
  authorizationId: 'd10f67f2-f471-44ea-b6d1-e1b963e11228',
  path: '/work/loopal', name: 'loopal', suggestedWorktreeName: 'loopal-task',
  git: { root: '/work/loopal', branch: 'main', dirty: false },
}

describe('main-process session directory picker', () => {
  beforeEach(() => {
    delete process.env.LOOPAL_DESKTOP_E2E_DIRECTORY_QUEUE
    delete process.env.LOOPAL_DESKTOP_E2E_HIDDEN
    delete electron.state.handler
    electron.ipcMain.handle.mockClear()
    electron.ipcMain.removeHandler.mockClear()
  })
  afterEach(() => {
    delete process.env.LOOPAL_DESKTOP_E2E_DIRECTORY_QUEUE
    delete process.env.LOOPAL_DESKTOP_E2E_HIDDEN
  })

  it('authorizes only a main-frame native selection', async () => {
    const mainFrame = {}
    const webContents = { mainFrame }
    const authorizeSessionDirectory = vi.fn(async () => selection)
    const registration = registerSessionDirectoryPicker(
      { webContents } as never,
      { authorizeSessionDirectory },
      { validateDirectory: async (path) => path, pickDirectory: async () => '/work/loopal' },
    )
    expect(electron.ipcMain.handle).toHaveBeenCalledWith(
      SELECT_SESSION_DIRECTORY_CHANNEL, expect.any(Function),
    )

    await expect(electron.state.handler!({ sender: {}, senderFrame: mainFrame }))
      .rejects.toThrow('restricted to the main window')
    await expect(electron.state.handler!(
      { sender: webContents, senderFrame: mainFrame }, '/forged/path',
    )).rejects.toThrow('does not accept renderer paths')
    await expect(electron.state.handler!({ sender: webContents, senderFrame: mainFrame }))
      .resolves.toEqual(selection)
    expect(authorizeSessionDirectory).toHaveBeenCalledWith('/work/loopal')

    registration.dispose()
    expect(electron.ipcMain.removeHandler).toHaveBeenCalledWith(
      SELECT_SESSION_DIRECTORY_CHANNEL,
    )
  })

  it('does not authorize a cancelled selection', async () => {
    const mainFrame = {}
    const webContents = { mainFrame }
    const authorizeSessionDirectory = vi.fn()
    registerSessionDirectoryPicker(
      { webContents } as never, { authorizeSessionDirectory },
      { validateDirectory: async (path) => path, pickDirectory: async () => undefined },
    )
    await expect(electron.state.handler!({ sender: webContents, senderFrame: mainFrame }))
      .resolves.toBeUndefined()
    expect(authorizeSessionDirectory).not.toHaveBeenCalled()
  })

  it('rejects unsafe native selections before backend authorization', async () => {
    const mainFrame = {}
    const webContents = { mainFrame }
    const authorizeSessionDirectory = vi.fn()
    registerSessionDirectoryPicker(
      { webContents } as never, { authorizeSessionDirectory }, {
        validateDirectory: async () => undefined,
        pickDirectory: async () => '/private/application-state',
      },
    )
    await expect(electron.state.handler!({ sender: webContents, senderFrame: mainFrame }))
      .rejects.toThrow('unavailable or unsafe')
    expect(authorizeSessionDirectory).not.toHaveBeenCalled()
  })

  it('rejects a safe subdirectory when its Git root is reserved', async () => {
    const mainFrame = {}
    const webContents = { mainFrame }
    const unsafeRoot = { ...selection, git: { ...selection.git, root: '/reserved/home' } }
    const authorizeSessionDirectory = vi.fn(async () => unsafeRoot)
    registerSessionDirectoryPicker(
      { webContents } as never, { authorizeSessionDirectory }, {
        validateDirectory: async (path) => path === '/work/loopal' ? path : undefined,
        pickDirectory: async () => '/work/loopal',
      },
    )
    await expect(electron.state.handler!({ sender: webContents, senderFrame: mainFrame }))
      .rejects.toThrow('Git repository is unavailable or unsafe')
    expect(authorizeSessionDirectory).toHaveBeenCalledWith('/work/loopal')
  })

  it('rejects a directory that changes while the backend inspects it', async () => {
    const mainFrame = {}
    const webContents = { mainFrame }
    const changed = { ...selection, path: '/reserved/replaced' }
    registerSessionDirectoryPicker(
      { webContents } as never,
      { authorizeSessionDirectory: async () => changed },
      {
        validateDirectory: async (path) => path,
        pickDirectory: async () => '/work/loopal',
      },
    )

    await expect(electron.state.handler!({ sender: webContents, senderFrame: mainFrame }))
      .rejects.toThrow('changed during authorization')
  })

  it('returns the validated canonical Git root when path spelling differs', async () => {
    const mainFrame = {}
    const webContents = { mainFrame }
    const differentlySpelled = {
      ...selection, git: { ...selection.git, root: '/WORK/LOOPAL' },
    }
    registerSessionDirectoryPicker(
      { webContents } as never,
      { authorizeSessionDirectory: async () => differentlySpelled },
      {
        validateDirectory: async (path) => path.toLocaleLowerCase(),
        pickDirectory: async () => '/WORK/LOOPAL',
      },
    )

    await expect(electron.state.handler!({ sender: webContents, senderFrame: mainFrame }))
      .resolves.toEqual(selection)
  })

  it('uses an explicit hidden-E2E queue without opening a native dialog', async () => {
    process.env.LOOPAL_DESKTOP_E2E_HIDDEN = '1'
    process.env.LOOPAL_DESKTOP_E2E_DIRECTORY_QUEUE = JSON.stringify(['/work/loopal', null])
    const mainFrame = {}
    const webContents = { mainFrame }
    const authorizeSessionDirectory = vi.fn(async () => selection)
    const native = vi.fn(async () => '/native')
    registerSessionDirectoryPicker(
      { webContents } as never, { authorizeSessionDirectory },
      { validateDirectory: async (path) => path, pickDirectory: native },
    )

    await electron.state.handler!({ sender: webContents, senderFrame: mainFrame })
    await expect(electron.state.handler!({ sender: webContents, senderFrame: mainFrame }))
      .resolves.toBeUndefined()
    expect(authorizeSessionDirectory).toHaveBeenCalledWith('/work/loopal')
    expect(native).not.toHaveBeenCalled()
  })
})
