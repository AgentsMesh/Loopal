import { expect, test } from '@playwright/test'
import { execFile, spawn } from 'node:child_process'
import { mkdir, readFile, readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { promisify } from 'node:util'
import {
  closeDesktop, launchDesktop, loopalBinaryPath, relaunchDesktop,
  shutdownDesktop, type DesktopFixture,
} from '../../support/electron/electron-fixture'
import { isolatedTestEnvironment } from '../../support/providers/mock-llm-fixture'
import { activeDetail, ready, runtimeTarget, send } from '../../support/runtime/llm-e2e-helpers'

const canary = 'LOOPAL_DESKTOP_E2E_SECRET_7c19f2a84d'
const placeholder = '<secret_ref:desktop_canary>'
const execFileAsync = promisify(execFile)

test('resolves a vault secret only inside a real Bash subprocess', async () => {
  let desktop = await launchDesktop('real', 'provider-secret-vault')
  let stopped = false
  try {
    await ready(desktop.page)
    await initializeVault(desktop)
    desktop = await relaunchDesktop(desktop)
    await ready(desktop.page)

    const target = await runtimeTarget(desktop.page)
    await desktop.page.evaluate(async (value) => window.loopalDesktop.controlAgent({
      target: value, command: { type: 'permission', mode: 'bypass' },
    }), target)
    await send(desktop.page, 'Exercise the production secret boundary')

    const conversation = desktop.page.getByTestId('conversation')
    await expect(conversation).toContainText(
      'The production secret boundary stayed redacted.', { timeout: 20_000 },
    )
    const tool = conversation.getByTestId('tool-invocation').filter({ hasText: 'printf' }).last()
    await expect(tool.getByLabel('Completed')).toBeVisible()
    await tool.locator(':scope > summary').click()
    await expect(tool.locator('.tool-output')).toContainText(`observed=${placeholder}`)
    await expect(conversation).not.toContainText(canary)
    const liveDetail = JSON.stringify(await activeDetail(desktop.page))
    expect(liveDetail).toContain(`observed=${placeholder}`)
    expect(liveDetail).not.toContain(canary)

    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(2)
    expect(requests[1]).toMatchObject({
      toolResultIds: ['secret-bash'], toolResultErrorIds: [], matched: true,
    })
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, unmatchedRequests: 0, verified: true,
    })

    desktop = await relaunchDesktop(desktop)
    await ready(desktop.page)
    await expect(desktop.page.getByTestId('conversation')).toContainText(`observed=${placeholder}`)
    await expect(desktop.page.getByTestId('conversation')).not.toContainText(canary)
    expect(JSON.stringify(await activeDetail(desktop.page))).not.toContain(canary)
    await assertPublicSettings(desktop)
    await shutdownDesktop(desktop)
    stopped = true
    await assertTreesExclude(canary, [desktop.home, desktop.project, join(desktop.root, 'user-data')])
  } finally {
    if (stopped) await desktop.cleanup()
    else await closeDesktop(desktop)
  }
})

async function initializeVault(desktop: DesktopFixture): Promise<void> {
  const ssh = join(desktop.home, '.ssh')
  await mkdir(ssh)
  await execFileAsync('ssh-keygen', [
    '-q', '-t', 'ed25519', '-N', '', '-C', 'loopal-desktop-e2e',
    '-f', join(ssh, 'id_ed25519'),
  ], { env: isolatedTestEnvironment({ HOME: desktop.home }) })
  await runLoopal(desktop, ['vaults', 'init'])
  await runLoopal(desktop, ['vault', 'set', 'desktop_canary'], `${canary}\n`)
}

async function runLoopal(
  desktop: DesktopFixture, args: string[], stdin?: string,
): Promise<void> {
  const child = spawn(loopalBinaryPath(), args, {
    cwd: desktop.project,
    env: isolatedTestEnvironment({ HOME: desktop.home }),
    stdio: ['pipe', 'pipe', 'pipe'],
  })
  let stderr = ''
  child.stderr.setEncoding('utf8').on('data', (chunk: string) => { stderr += chunk })
  child.stdout.resume()
  child.stdin.end(stdin)
  const code = await new Promise<number | null>((resolve, reject) => {
    const timer = setTimeout(() => {
      child.kill('SIGKILL')
      reject(new Error(`loopal ${args.join(' ')} timed out`))
    }, 15_000)
    child.once('error', (error) => { clearTimeout(timer); reject(error) })
    child.once('close', (value) => { clearTimeout(timer); resolve(value) })
  })
  if (code !== 0) throw new Error(`loopal ${args.join(' ')} failed (${code}): ${stderr}`)
}

async function assertPublicSettings(desktop: DesktopFixture): Promise<void> {
  const projections = await desktop.page.evaluate(async () => Promise.all([
    window.loopalDesktop.getLoopalSettings('local-workspace'),
    window.loopalDesktop.listMcpServers('local-workspace'),
    window.loopalDesktop.getMetaHubSettings(),
  ]))
  expect(JSON.stringify(projections)).not.toContain(canary)
  await desktop.page.getByRole('button', { name: 'Settings' }).click()
  await expect(desktop.page.getByTestId('settings-pane')).toBeVisible()
  expect(await desktop.page.getByTestId('settings-pane').textContent()).not.toContain(canary)
}

async function assertTreesExclude(secret: string, roots: string[]): Promise<void> {
  const needle = Buffer.from(secret)
  for (const root of roots) {
    for (const path of await filesBelow(root)) {
      expect((await readFile(path)).includes(needle), `plaintext secret persisted at ${path}`)
        .toBe(false)
    }
  }
}

async function filesBelow(root: string): Promise<string[]> {
  const entries = await readdir(root, { withFileTypes: true })
  const nested = await Promise.all(entries.map(async (entry) => {
    const path = join(root, entry.name)
    if (entry.isDirectory()) return filesBelow(path)
    return entry.isFile() ? [path] : []
  }))
  return nested.flat()
}
