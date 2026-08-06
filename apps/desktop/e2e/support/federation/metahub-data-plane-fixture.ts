import { spawn, type ChildProcess } from 'node:child_process'
import { mkdir } from 'node:fs/promises'
import { join } from 'node:path'
import { createInterface } from 'node:readline'
import { loopalBinaryPath, type DesktopFixture } from '../electron/electron-fixture'
import { isolatedTestEnvironment } from '../providers/mock-llm-fixture'
import { HubProbe, registerCapableUiBeforeReady } from './desktop-ready-ui'

export { HubProbe } from './desktop-ready-ui'

export interface MetaHubProcess {
  readonly child: ChildProcess
  readonly address: string
  readonly token: string
}

export interface RemoteHubProcess {
  readonly child: ChildProcess
  readonly probe: HubProbe
}

export async function startMetaHub(): Promise<MetaHubProcess> {
  const child = spawn(loopalBinaryPath(), [
    '--meta-hub', '127.0.0.1:0', '--meta-hub-parent-pid', String(process.pid),
  ], { env: isolatedTestEnvironment(), stdio: ['ignore', 'pipe', 'pipe'] })
  child.stderr?.resume()
  try {
    const value = await waitForPrefixedJson(child, 'LOOPAL_METAHUB ', 10_000)
    return { child, address: String(value.address), token: String(value.token) }
  } catch (error) {
    await stopProcess(child)
    throw error
  }
}

export async function startRemoteHub(
  desktop: DesktopFixture, meta: MetaHubProcess, name = 'hub-b',
): Promise<RemoteHubProcess> {
  const home = join(desktop.root, `${name}-home`)
  await mkdir(home, { recursive: true })
  const child = spawn(loopalBinaryPath(), [
    'desktop', 'serve', '--parent-pid', String(process.pid),
    '--join-hub', meta.address, '--hub-name', name,
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
    const probe = await registerCapableUiBeforeReady(child)
    return { child, probe }
  } catch (error) {
    await stopProcess(child)
    throw error
  }
}

async function waitForPrefixedJson(
  child: ChildProcess, prefix: string, timeout: number,
  accept: (value: Record<string, unknown>) => boolean = () => true,
): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    const lines = createInterface({ input: child.stdout!, crlfDelay: Infinity })
    const timer = setTimeout(() => finish(new Error(`Timed out waiting for ${prefix.trim()}`)), timeout)
    const finish = (error?: Error, value?: Record<string, unknown>): void => {
      clearTimeout(timer); lines.close(); child.off('exit', exited)
      error ? reject(error) : resolve(value!)
    }
    const exited = (): void => finish(new Error('Loopal fixture exited before handshake'))
    child.once('exit', exited)
    lines.on('line', (line) => {
      if (!line.startsWith(prefix)) return
      try {
        const value = JSON.parse(line.slice(prefix.length)) as Record<string, unknown>
        if (accept(value)) finish(undefined, value)
      } catch { finish(new Error('Invalid Loopal fixture handshake')) }
    })
  })
}

export async function stopProcess(child: ChildProcess): Promise<void> {
  if (!child.pid || child.exitCode !== null || child.signalCode !== null) return
  child.kill('SIGTERM')
  await new Promise<void>((resolve) => {
    const timer = setTimeout(() => { child.kill('SIGKILL'); resolve() }, 3_000)
    child.once('exit', () => { clearTimeout(timer); resolve() })
  })
}
