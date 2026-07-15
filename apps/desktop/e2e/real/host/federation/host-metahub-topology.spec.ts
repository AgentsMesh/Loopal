import { spawn, type ChildProcess } from 'node:child_process'
import { createInterface } from 'node:readline'
import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'
import { expect, test } from '@playwright/test'
import {
  closeDesktop,
  launchDesktop,
  loopalBinaryPath,
  type DesktopFixture,
} from '../../../support/electron/electron-fixture'
import { isolatedTestEnvironment } from '../../../support/providers/mock-llm-fixture'

interface MetaFixture {
  readonly child: ChildProcess
  readonly address: string
  readonly token: string
}

test('streams remote Hub joins and leaves into the central qualified Agent topology', async () => {
  const meta = await startMetaHub()
  let desktop: DesktopFixture | undefined
  let remote: ChildProcess | undefined
  try {
    desktop = await launchDesktop('real')
    const target = await desktop.page.evaluate(async ({ address, token }) => {
      const api = window.loopalDesktop
      const bootstrap = await api.bootstrap()
      const sessionId = bootstrap.activeSessionId!
      const runtime = bootstrap.runtimes.find((value) => value.sessionId === sessionId)!
      await api.updateMetaHubSettings({
        address, hubName: 'hub-a', token, joinOnStart: false, startLocalOnLaunch: false,
      })
      const target = { sessionId, runtimeId: runtime.id, generation: runtime.generation }
      await api.joinMetaHub(target)
      return target
    }, { address: meta.address, token: meta.token })
    await desktop.page.getByRole('button', { name: 'Federation', exact: true }).click()
    await expect(desktop.page.getByTestId('primary-workspace'))
      .toHaveAttribute('data-workspace', 'federation')
    await expect(desktop.page.getByRole('tab', { name: 'Agents', exact: true })).toHaveCount(0)
    remote = await startRemoteHub(desktop, meta)

    const remoteNode = desktop.page.locator('[data-agent-id="hub-b/main"]')
    await expect(remoteNode).toBeVisible({ timeout: 15_000 })
    await expect(remoteNode).toContainText('hub-b')
    const projected = await desktop.page.evaluate(async (sessionId) => (
      (await window.loopalDesktop.openSession(sessionId)).agents
        .find((agent) => agent.id === 'hub-b/main')
    ), target.sessionId)
    expect(projected).toMatchObject({
      qualifiedName: 'hub-b/main', hubPath: ['hub-b'], controllable: false,
    })

    await stop(remote)
    remote = undefined
    await expect(remoteNode).toHaveCount(0, { timeout: 15_000 })
    await desktop.page.evaluate(async (value) => {
      await window.loopalDesktop.disconnectMetaHub(value)
    }, target)
    await expect(desktop.page.getByTestId('federation-workspace')).toContainText(
      'Start a Federation for your Loopal sessions.',
    )
  } finally {
    if (remote) await stop(remote)
    if (desktop) await closeDesktop(desktop)
    await stop(meta.child)
  }
})

async function startMetaHub(): Promise<MetaFixture> {
  const child = spawn(loopalBinaryPath(), [
    '--meta-hub', '127.0.0.1:0', '--meta-hub-parent-pid', String(process.pid),
  ], { env: isolatedTestEnvironment(), stdio: ['ignore', 'pipe', 'pipe'] })
  child.stderr?.resume()
  try {
    const value = await waitForLine(child, 'LOOPAL_METAHUB ', 10_000)
    return {
      child,
      address: String(value.address),
      token: String(value.token),
    }
  } catch (error) {
    await stop(child)
    throw error
  }
}

async function startRemoteHub(desktop: DesktopFixture, meta: MetaFixture): Promise<ChildProcess> {
  const home = join(desktop.root, 'remote-home')
  await mkdir(home)
  const child = spawn(loopalBinaryPath(), [
    'desktop', 'serve', '--parent-pid', String(process.pid), '--ephemeral',
    '--join-hub', meta.address, '--hub-name', 'hub-b',
  ], {
    cwd: desktop.project,
    env: isolatedTestEnvironment({
      HOME: home,
      LOOPAL_META_HUB_TOKEN: meta.token,
      ANTHROPIC_API_KEY: desktop.llm!.apiKey,
      ANTHROPIC_BASE_URL: desktop.llm!.baseUrl,
      LOOPAL_OTEL_ENABLED: 'false',
      LOOPAL_MCP_STARTUP_WAIT_SECS: '1',
    }),
    stdio: ['ignore', 'pipe', 'pipe'],
  })
  child.stderr?.resume()
  try {
    await waitForLine(child, 'LOOPAL_DESKTOP ', 20_000, (value) => value.phase === 'ready')
    return child
  } catch (error) {
    await stop(child)
    throw error
  }
}

async function waitForLine(
  child: ChildProcess,
  prefix: string,
  timeout: number,
  accept: (value: Record<string, unknown>) => boolean = () => true,
): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    const lines = createInterface({ input: child.stdout!, crlfDelay: Infinity })
    let settled = false
    const timer = setTimeout(() => done(new Error(`Timed out waiting for ${prefix.trim()}`)), timeout)
    const done = (error?: Error, value?: Record<string, unknown>): void => {
      if (settled) return
      settled = true
      clearTimeout(timer); lines.close(); child.off('exit', exited); child.off('error', failed)
      error ? reject(error) : resolve(value!)
    }
    const exited = (): void => done(new Error('Loopal fixture exited before ready'))
    const failed = (error: Error): void => done(error)
    child.once('exit', exited)
    child.once('error', failed)
    lines.on('line', (line) => {
      if (!line.startsWith(prefix)) return
      try {
        const value = JSON.parse(line.slice(prefix.length)) as Record<string, unknown>
        if (accept(value)) done(undefined, value)
      } catch { done(new Error('Invalid Loopal fixture handshake')) }
    })
  })
}

async function stop(child: ChildProcess): Promise<void> {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) return
  child.kill('SIGTERM')
  if (await waitForExit(child, 3_000)) return
  child.kill('SIGKILL')
  await waitForExit(child, 1_000)
}

function waitForExit(child: ChildProcess, timeout: number): Promise<boolean> {
  if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve(true)
  return new Promise((resolve) => {
    const timer = setTimeout(() => { child.off('exit', exited); resolve(false) }, timeout)
    const exited = (): void => {
      clearTimeout(timer)
      resolve(true)
    }
    child.once('exit', exited)
  })
}
