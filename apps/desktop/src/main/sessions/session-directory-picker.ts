import { dialog, ipcMain, type BrowserWindow, type IpcMainInvokeEvent } from 'electron'
import { isAbsolute } from 'node:path'
import { toDisposable, type IDisposable } from '../../base/common/lifecycle'
import {
  SessionDirectorySelectionSchema, type SessionDirectorySelection,
} from '../../shared/contracts'
import { SELECT_SESSION_DIRECTORY_CHANNEL } from '../../shared/protocol/renderer-protocol'

export interface SessionDirectoryAuthorizer {
  authorizeSessionDirectory(path: string): Promise<SessionDirectorySelection>
}

type DirectoryPicker = () => Promise<string | undefined>
interface SessionDirectoryPickerOptions {
  readonly validateDirectory: (path: string) => Promise<string | undefined>
  readonly pickDirectory?: DirectoryPicker
}

export function registerSessionDirectoryPicker(
  window: BrowserWindow,
  authorizer: SessionDirectoryAuthorizer,
  options: SessionDirectoryPickerOptions,
): IDisposable {
  const queued = e2eDirectoryPicker(process.env)
  const picker = queued ?? options.pickDirectory ?? nativePicker(window)
  const handler = async (event: IpcMainInvokeEvent, ...args: unknown[]): Promise<unknown> => {
    if (event.sender !== window.webContents || event.senderFrame !== window.webContents.mainFrame) {
      throw new Error('Directory selection is restricted to the main window')
    }
    if (args.length !== 0) throw new Error('Directory selection does not accept renderer paths')
    const path = await picker()
    if (!path) return undefined
    const validated = await options.validateDirectory(path)
    if (!validated) throw new Error('The selected session directory is unavailable or unsafe')
    const selection = SessionDirectorySelectionSchema.parse(
      await authorizer.authorizeSessionDirectory(validated),
    )
    const selectedPath = await options.validateDirectory(selection.path)
    if (!selectedPath || selectedPath !== validated) {
      throw new Error('The selected session directory changed during authorization')
    }
    if (selection.git) {
      const gitRoot = await options.validateDirectory(selection.git.root)
      if (!gitRoot) {
        throw new Error('The selected Git repository is unavailable or unsafe')
      }
      return SessionDirectorySelectionSchema.parse({
        ...selection, path: selectedPath, git: { ...selection.git, root: gitRoot },
      })
    }
    return SessionDirectorySelectionSchema.parse({ ...selection, path: selectedPath })
  }
  ipcMain.handle(SELECT_SESSION_DIRECTORY_CHANNEL, handler)
  return toDisposable(() => ipcMain.removeHandler(SELECT_SESSION_DIRECTORY_CHANNEL))
}

function nativePicker(window: BrowserWindow): DirectoryPicker {
  return async () => {
    const result = await dialog.showOpenDialog(window, {
      properties: ['openDirectory', 'createDirectory'],
    })
    return result.canceled ? undefined : result.filePaths[0]
  }
}

function e2eDirectoryPicker(env: NodeJS.ProcessEnv): DirectoryPicker | undefined {
  const raw = env.LOOPAL_DESKTOP_E2E_DIRECTORY_QUEUE
  if (env.LOOPAL_DESKTOP_E2E_HIDDEN !== '1' || !raw) return undefined
  const parsed: unknown = JSON.parse(raw)
  if (!Array.isArray(parsed) || parsed.some((value) => value !== null
    && (typeof value !== 'string' || !isAbsolute(value)))) {
    throw new Error('LOOPAL_DESKTOP_E2E_DIRECTORY_QUEUE must contain absolute paths or null')
  }
  const queue = [...parsed] as (string | null)[]
  return async () => queue.shift() ?? undefined
}
