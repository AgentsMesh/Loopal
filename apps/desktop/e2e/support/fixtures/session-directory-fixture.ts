import { execFile as execFileCallback } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { promisify } from 'node:util'
import { type DesktopFixture } from '../electron/electron-fixture'
import { seedWorkspace } from './fixture-loader'

const execFile = promisify(execFileCallback)

export interface SessionDirectoryFixture {
  readonly path: string
  readonly git: boolean
  readonly branch?: string
}

export async function createSessionDirectory(
  desktop: DesktopFixture,
  name: string,
  git: boolean,
): Promise<SessionDirectoryFixture> {
  const parent = join(desktop.root, 'session-directories')
  const path = join(parent, name)
  await mkdir(parent, { recursive: true })
  await seedWorkspace(path, desktop.root)
  await writeFile(join(path, 'session-fixture.txt'), `${name}\n`)
  if (!git) return { path, git }
  await runGit(desktop, path, ['init', '-q', '--initial-branch=main'])
  await runGit(desktop, path, ['add', '.'])
  await runGit(desktop, path, ['commit', '-qm', 'session fixture'])
  return { path, git, branch: 'main' }
}

export async function queueSessionDirectories(
  desktop: DesktopFixture,
  paths: readonly (string | null)[],
): Promise<void> {
  await desktop.app.evaluate(({ dialog }, queue) => {
    const pending = [...queue]
    dialog.showOpenDialog = (async () => {
      const selected = pending.shift()
      return selected
        ? { canceled: false, filePaths: [selected] }
        : { canceled: true, filePaths: [] }
    }) as typeof dialog.showOpenDialog
  }, paths)
}

export async function gitOutput(
  desktop: DesktopFixture,
  cwd: string,
  args: readonly string[],
): Promise<string> {
  const result = await execFile('git', args, { cwd, env: gitEnvironment(desktop) })
  return result.stdout.trim()
}

async function runGit(
  desktop: DesktopFixture,
  cwd: string,
  args: readonly string[],
): Promise<void> {
  await execFile('git', args, { cwd, env: gitEnvironment(desktop) })
}

function gitEnvironment(desktop: DesktopFixture): NodeJS.ProcessEnv {
  return {
    ...process.env,
    HOME: desktop.home,
    GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: '/dev/null',
    GIT_AUTHOR_NAME: 'Loopal Desktop E2E',
    GIT_COMMITTER_NAME: 'Loopal Desktop E2E',
    GIT_AUTHOR_EMAIL: 'desktop-e2e@loopal.local',
    GIT_COMMITTER_EMAIL: 'desktop-e2e@loopal.local',
    GIT_AUTHOR_DATE: '2000-01-01T00:00:00Z',
    GIT_COMMITTER_DATE: '2000-01-01T00:00:00Z',
  }
}
