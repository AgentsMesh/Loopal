import { _electron, type ElectronApplication, type Page } from '@playwright/test'
import { execFile as execFileCallback } from 'node:child_process'
import { mkdtemp, mkdir, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { promisify } from 'node:util'
import {
  isolatedTestEnvironment, startMockLlm, type MockLlmFixture,
} from '../providers/mock-llm-fixture'
import { loadLlmScenario, seedWorkspace } from '../fixtures/fixture-loader'
import {
  configureProvider, type E2eProvider, persistedDesktopEnvironment, providerEnvironment,
} from '../providers/provider-e2e-fixture'
export { waitForHostStatus } from '../runtime/host-status'
const execFile = promisify(execFileCallback)

export interface DesktopFixture {
  readonly app: ElectronApplication
  readonly page: Page
  readonly root: string
  readonly home: string
  readonly project: string
  readonly backend: 'fake' | 'real'
  readonly provider: E2eProvider
  readonly llm?: MockLlmFixture
  cleanup(): Promise<void>
}

export async function launchDesktop(
  backend: 'fake' | 'real', scenario?: string,
  environment: Readonly<Record<string, string>> = {},
  provider: E2eProvider = 'anthropic',
  workspaceFixture = 'basic',
): Promise<DesktopFixture> {
  const root = await mkdtemp(join(tmpdir(), 'loopal-desktop-e2e-'))
  const home = join(root, 'home')
  const userData = join(root, 'user-data')
  const project = join(root, 'project')
  let app: ElectronApplication | undefined
  let llm: MockLlmFixture | undefined
  try {
    await Promise.all([mkdir(home), mkdir(userData)])
    const env = isolatedTestEnvironment({
      ...environment,
      HOME: home,
      LOOPAL_DESKTOP_CWD: project,
      LOOPAL_DESKTOP_E2E_HIDDEN: '1',
    })
    if (backend === 'fake') {
      await mkdir(project)
      env.LOOPAL_DESKTOP_BACKEND = 'fake'
    } else {
      await seedWorkspace(project, root, workspaceFixture)
      await seedProject(project, root)
      delete env.LOOPAL_DESKTOP_BACKEND
      delete env.LOOPAL_DESKTOP_BINARY
      delete env.ELECTRON_RENDERER_URL
      const calls = await loadLlmScenario(scenario ?? 'default-ok', {
        PROJECT: project, HOME: home, ROOT: root,
      })
      llm = await startMockLlm(root, calls)
      await configureProvider(home, llm, provider, environment.LOOPAL_DESKTOP_E2E_USER_SETTINGS)
      Object.assign(env, providerEnvironment(llm, provider))
      env.LOOPAL_DESKTOP_BINARY_RUNFILE = loopalBinaryRunfile()
      env.LOOPAL_MCP_STARTUP_WAIT_SECS = '1'
    }
    app = await _electron.launch({
      args: [runfile('apps/desktop/out/main/index.cjs'), `--user-data-dir=${userData}`],
      env,
      timeout: 30_000,
    })
    const page = await app.firstWindow({ timeout: 30_000 })
    await page.getByTestId('workbench').waitFor({ timeout: 10_000 })
    await stabilizeSystemLanguage(page)
    return {
      app,
      page,
      root,
      home,
      project,
      backend,
      provider,
      ...(llm ? { llm } : {}),
      cleanup: async () => {
        await llm?.stop()
        await rm(root, { recursive: true, force: true })
      },
    }
  } catch (error) {
    if (app) await stopApp(app)
    await llm?.stop()
    await rm(root, { recursive: true, force: true })
    throw error
  }
}

export async function relaunchDesktop(fixture: DesktopFixture): Promise<DesktopFixture> {
  await shutdownDesktop(fixture)
  const env = persistedDesktopEnvironment(
    fixture.backend, fixture.home, fixture.project, loopalBinaryRunfile(),
    fixture.llm, fixture.provider,
  )
  let app: ElectronApplication | undefined
  try {
    app = await _electron.launch({
      args: [runfile('apps/desktop/out/main/index.cjs'), `--user-data-dir=${join(fixture.root, 'user-data')}`],
      env,
      timeout: 30_000,
    })
    const page = await app.firstWindow({ timeout: 30_000 })
    await page.getByTestId('workbench').waitFor({ timeout: 10_000 })
    await stabilizeSystemLanguage(page)
    return { ...fixture, app, page }
  } catch (error) {
    if (app) await stopApp(app)
    throw error
  }
}

async function seedProject(project: string, root: string): Promise<void> {
  if (process.platform !== 'win32') {
    const { symlink } = await import('node:fs/promises')
    await symlink(join(root, 'outside.txt'), join(project, 'escape-link'))
  }
  const options = { cwd: project, env: isolatedTestEnvironment({
    HOME: join(root, 'home'), GIT_CONFIG_NOSYSTEM: '1', GIT_CONFIG_GLOBAL: '/dev/null',
    GIT_AUTHOR_NAME: 'Loopal Desktop E2E', GIT_COMMITTER_NAME: 'Loopal Desktop E2E',
    GIT_AUTHOR_EMAIL: 'desktop-e2e@loopal.local', GIT_COMMITTER_EMAIL: 'desktop-e2e@loopal.local',
    GIT_AUTHOR_DATE: '2000-01-01T00:00:00Z', GIT_COMMITTER_DATE: '2000-01-01T00:00:00Z',
  }) }
  await execFile('git', ['init', '-q', '--initial-branch=main'], options)
  await execFile('git', ['add', 'README.md', 'src/main.rs'], options)
  await execFile('git', ['commit', '-qm', 'fixture'], options)
}

export async function closeDesktop(fixture: DesktopFixture): Promise<void> {
  try {
    await shutdownDesktop(fixture)
  } finally {
    await fixture.cleanup()
  }
}

export async function shutdownDesktop(fixture: DesktopFixture): Promise<void> {
  await stopApp(fixture.app)
}

async function stopApp(app: ElectronApplication): Promise<void> {
  const child = app.process()
  let timer: NodeJS.Timeout | undefined
  const deadline = new Promise<void>((resolve) => {
    timer = setTimeout(resolve, 5_000)
  })
  try {
    await Promise.race([app.close().catch(() => undefined), deadline])
  } finally {
    clearTimeout(timer)
    if (child.exitCode !== null) return
    if (process.platform === 'win32' || !child.pid) {
      child.kill('SIGKILL')
      return
    }
    try {
      process.kill(-child.pid, 'SIGKILL')
    } catch {
      child.kill('SIGKILL')
    }
  }
}

function runfile(relative: string): string {
  const testSrcDir = process.env.TEST_SRCDIR
  const workspace = process.env.TEST_WORKSPACE
  if (testSrcDir && workspace) return join(testSrcDir, workspace, relative)
  const here = dirname(fileURLToPath(import.meta.url))
  return resolve(here, '../../../../..', relative)
}

function loopalBinaryRunfile(): string {
  const executable = process.platform === 'win32' ? 'loopal.exe' : 'loopal'
  return `${process.env.TEST_WORKSPACE ?? '_main'}/${executable}`
}

export function loopalBinaryPath(): string {
  const executable = process.platform === 'win32' ? 'loopal.exe' : 'loopal'
  return runfile(executable)
}
async function stabilizeSystemLanguage(page: Page): Promise<void> {
  await page.evaluate(() => {
    Object.defineProperties(navigator, {
      language: { configurable: true, get: () => 'en-US' },
      languages: { configurable: true, get: () => ['en-US'] },
    })
    window.dispatchEvent(new Event('languagechange'))
  })
  const preferences = await page.evaluate(() => window.loopalDesktop.getDesktopPreferences())
  if (preferences.locale === 'system')
    await page.waitForFunction(() => document.documentElement.lang === 'en')
}
