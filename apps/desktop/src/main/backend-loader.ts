import { app, dialog } from 'electron'
import { dirname, join } from 'node:path'
import { type DesktopBackend } from '../platform/loopal-backend/common/backend'
import { type SessionDirectorySelection } from '../shared/contracts'
import { FakeDesktopBackend } from '../platform/loopal-backend/node/fake/fake-backend'
import { LoopalDesktopBackend } from '../platform/loopal-backend/node/backend/loopal-backend'
import { UnavailableDesktopBackend } from '../platform/loopal-backend/node/unavailable/unavailable-backend'
import {
  resolveDesktopCwd, resolveLoopalBinary, useFakeBackend,
} from './app/runtime-mode'
import { authorizePackagedWorkspace } from './sessions/workspace-authorization'

export interface AppBackend extends DesktopBackend {
  dispose(): void
  shutdown?(): Promise<void>
  authorizeSessionDirectory(path: string): Promise<SessionDirectorySelection>
}

export async function loadDesktopBackend(): Promise<AppBackend> {
  const preferencesPath = join(app.getPath('userData'), 'desktop-preferences.json')
  if (useFakeBackend(app.isPackaged, process.env)) {
    return new FakeDesktopBackend(undefined, preferencesPath)
  }
  const binaryPath = resolveLoopalBinary({
    isPackaged: app.isPackaged,
    env: process.env,
    resourcesPath: process.resourcesPath,
    platform: process.platform,
    cwd: process.cwd(),
  })
  if (!binaryPath) {
    return new UnavailableDesktopBackend(
      'Loopal Desktop Host is unavailable. Build //:loopal and launch the Bazel desktop target.',
    )
  }
  let cwd = app.isPackaged
    ? undefined
    : resolveDesktopCwd(false, process.env, process.cwd())
  if (!cwd) {
    const authorization = await authorizePackagedWorkspace({
      userDataPath: app.getPath('userData'),
      homePath: app.getPath('home'),
      applicationPaths: [app.getAppPath(), process.resourcesPath, dirname(app.getPath('exe'))],
      selectDirectory: async () => {
        const result = await dialog.showOpenDialog({
          title: 'Open a Loopal workspace', properties: ['openDirectory'],
        })
        return result.canceled ? undefined : result.filePaths[0]
      },
    })
    if (!authorization.ok) return new UnavailableDesktopBackend(authorization.reason)
    cwd = authorization.path
  }
  return new LoopalDesktopBackend({
    binaryPath,
    cwd,
    parentPid: process.pid,
    sessionStatePath: join(app.getPath('userData'), 'session-lifecycle.json'),
    metaHubSettingsPath: join(app.getPath('userData'), 'metahub-settings.json'),
    desktopPreferencesPath: preferencesPath,
  })
}
