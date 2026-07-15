import { app, BrowserWindow, session, shell } from 'electron'
import { dirname, join } from 'node:path'
import { DisposableStore } from '../base/common/lifecycle'
import { monitorParent } from './app/parent-liveness'
import { isSafeExternalUrl, mainWindowPolicy, registerRendererServer } from './app/renderer-server'
import {
  keepE2eWindowHidden,
  resolveRendererUrl,
} from './app/runtime-mode'
import { registerImagePicker } from './media/image-picker'
import { registerSessionDirectoryPicker } from './sessions/session-directory-picker'
import { validateWorkspaceSelection } from './sessions/workspace-authorization'
import { loadDesktopBackend, type AppBackend } from './backend-loader'
const appDisposables = new DisposableStore()
const windowDisposables = new Map<number, DisposableStore>()
let mainWindow: BrowserWindow | undefined
let appBackend: AppBackend | undefined
let quitInProgress: Promise<void> | undefined
let readyToQuit = false
const keepWindowsHidden = keepE2eWindowHidden(app.isPackaged, process.env)
if (keepWindowsHidden && process.platform === 'darwin') app.setActivationPolicy('accessory')
if (!app.requestSingleInstanceLock()) {
  app.quit()
} else {
  if (keepWindowsHidden) appDisposables.add(monitorParent(process.ppid, () => app.quit()))
  app.on('second-instance', () => {
    if (keepWindowsHidden) return
    if (!mainWindow) {
      if (appBackend) {
        void createWindow(appBackend)
      }
      return
    }
    if (mainWindow.isMinimized()) {
      mainWindow.restore()
    }
    mainWindow.focus()
  })

  void app.whenReady().then(async () => {
    appBackend = await loadDesktopBackend()
    appDisposables.add(appBackend)
    configureSessionSecurity()
    await createWindow(appBackend)

    app.on('activate', () => {
      if (BrowserWindow.getAllWindows().length === 0) {
        void createWindow(appBackend!)
      }
    })
  })

  app.on('window-all-closed', () => {
    if (process.platform !== 'darwin') {
      app.quit()
    }
  })

  app.on('before-quit', (event) => {
    if (readyToQuit) {
      return
    }
    event.preventDefault()
    if (quitInProgress) {
      return
    }
    for (const store of windowDisposables.values()) {
      store.dispose()
    }
    windowDisposables.clear()
    quitInProgress = shutdownApplication().finally(() => {
      readyToQuit = true
      app.quit()
    })
  })
}
async function shutdownApplication(): Promise<void> {
  try {
    if (appBackend?.shutdown) {
      await Promise.race([
        appBackend.shutdown(),
        new Promise<void>((resolve) => setTimeout(resolve, 5_000)),
      ])
    }
  } finally {
    appDisposables.dispose()
  }
}
async function createWindow(backend: AppBackend): Promise<void> {
  const window = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 900,
    minHeight: 620,
    show: false,
    skipTaskbar: keepWindowsHidden,
    title: 'Loopal Desktop',
    backgroundColor: '#0b0d12',
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'default',
    webPreferences: {
      preload: join(__dirname, '../preload/index.cjs'),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      webSecurity: true,
      backgroundThrottling: !keepWindowsHidden,
    },
  })
  mainWindow = window
  const store = new DisposableStore()
  windowDisposables.set(window.id, store)
  store.add(registerRendererServer(backend, mainWindowPolicy(window.webContents)))
  store.add(registerImagePicker(window))
  store.add(registerSessionDirectoryPicker(window, backend, {
    validateDirectory: (path) => validateWorkspaceSelection(path, {
      userDataPath: app.getPath('userData'), homePath: app.getPath('home'),
      applicationPaths: [app.getAppPath(), process.resourcesPath, dirname(app.getPath('exe'))],
    }),
  }))
  window.webContents.setWindowOpenHandler(({ url }) => {
    if (isSafeExternalUrl(url)) {
      void shell.openExternal(url)
    }
    return { action: 'deny' }
  })
  window.webContents.on('will-navigate', (event, url) => {
    if (url !== window.webContents.getURL()) {
      event.preventDefault()
    }
  })
  if (!keepWindowsHidden) window.once('ready-to-show', () => window.show())
  window.once('closed', () => {
    store.dispose()
    windowDisposables.delete(window.id)
    if (mainWindow === window) {
      mainWindow = undefined
    }
  })
  const rendererUrl = resolveRendererUrl(app.isPackaged, process.env)
  if (rendererUrl) {
    await window.loadURL(rendererUrl)
  } else {
    await window.loadFile(join(__dirname, '../renderer/index.html'))
  }
}
function configureSessionSecurity(): void {
  session.defaultSession.setPermissionRequestHandler(
    (_webContents, _permission, callback) => callback(false),
  )
  session.defaultSession.setPermissionCheckHandler(() => false)
}
