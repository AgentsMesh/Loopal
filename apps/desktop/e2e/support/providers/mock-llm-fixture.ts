import { spawn, type ChildProcess } from 'node:child_process'
import { writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { createInterface } from 'node:readline'
import { fileURLToPath } from 'node:url'

const apiKey = 'loopal-desktop-e2e'
const handshakePrefix = 'LOOPAL_MOCK_LLM '

export interface MockLlmRequest {
  readonly sequence: number
  readonly protocol: string
  readonly model: string
  readonly messageCount: number
  readonly toolCount: number
  readonly toolNames: readonly string[]
  readonly toolResultIds: readonly string[]
  readonly toolResultErrorIds: readonly string[]
  readonly assistantBlockTypes: readonly string[]
  readonly serverBlockCount: number
  readonly imageBlockCount: number
  readonly lastUserText: string
  readonly hasSystem: boolean
  readonly thinkingEnabled: boolean
  readonly stream: boolean
  readonly maxTokens: number
  readonly apiKeyPresent: boolean
  readonly protocolVersionPresent: boolean
  readonly matched: boolean
}

export interface MockLlmState {
  readonly name: string
  readonly served: number
  readonly remaining: number
  readonly requestCount: number
  readonly unmatchedRequests: number
  readonly inFlight: number
  readonly clientDisconnects: number
  readonly scriptedDisconnects: number
  readonly verified: boolean
}

export interface MockLlmFixture {
  readonly child: ChildProcess
  readonly baseUrl: string
  readonly apiKey: string
  requests(): Promise<readonly MockLlmRequest[]>
  state(): Promise<MockLlmState>
  stop(): Promise<void>
}

export async function startMockLlm(root: string, scenario: unknown): Promise<MockLlmFixture> {
  const path = join(root, 'mock-llm-scenario.json')
  await writeFile(path, JSON.stringify(scenario))
  const child = spawn(mockLlmBinary(), [
    '--scenario', path, '--api-key', apiKey,
  ], { stdio: ['ignore', 'pipe', 'pipe'] })
  child.stderr?.resume()
  let ready: Record<string, unknown>
  try {
    ready = await waitForReady(child)
  } catch (error) {
    await stop(child)
    throw error
  }
  const baseUrl = String(ready.baseUrl)
  return {
    child,
    baseUrl,
    apiKey,
    requests: () => getJson(`${baseUrl}/__mock/requests`),
    state: () => getJson(`${baseUrl}/__mock/state`),
    stop: () => stop(child),
  }
}

export function mockProviderEnvironment(llm: MockLlmFixture): Record<string, string> {
  return {
    ANTHROPIC_API_KEY: llm.apiKey,
    ANTHROPIC_BASE_URL: llm.baseUrl,
    LOOPAL_OTEL_ENABLED: 'false',
  }
}

export function isolatedTestEnvironment(
  overrides: Readonly<Record<string, string>> = {},
): Record<string, string> {
  const env = Object.fromEntries(
    Object.entries(process.env).filter((entry): entry is [string, string] => entry[1] !== undefined),
  )
  const exact = new Set([
    'ELECTRON_RENDERER_URL', 'ELECTRON_RUN_AS_NODE', 'NODE_OPTIONS',
    'HTTP_PROXY', 'HTTPS_PROXY', 'ALL_PROXY', 'NO_PROXY',
    'http_proxy', 'https_proxy', 'all_proxy', 'no_proxy',
  ])
  for (const name of Object.keys(env)) {
    if (exact.has(name) || ['LOOPAL_', 'ANTHROPIC_', 'OPENAI_', 'GOOGLE_', 'OTEL_']
      .some((prefix) => name.startsWith(prefix))) delete env[name]
  }
  return { ...env, ...overrides }
}

function mockLlmBinary(): string {
  const configured = process.env.LOOPAL_MOCK_LLM_BINARY
    ?? 'crates/loopal-mock-llm/loopal-mock-llm'
  const testSrcDir = process.env.TEST_SRCDIR
  const workspace = process.env.TEST_WORKSPACE
  if (testSrcDir && workspace) return join(testSrcDir, workspace, configured)
  const here = dirname(fileURLToPath(import.meta.url))
  return resolve(here, '../../../../..', 'bazel-bin', configured)
}

function waitForReady(child: ChildProcess): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    const lines = createInterface({ input: child.stdout!, crlfDelay: Infinity })
    let settled = false
    const timer = setTimeout(() => done(new Error('Mock LLM did not become ready')), 10_000)
    const done = (error?: Error, value?: Record<string, unknown>): void => {
      if (settled) return
      settled = true
      clearTimeout(timer); lines.close(); child.off('exit', exited); child.off('error', failed)
      error ? reject(error) : resolve(value!)
    }
    const exited = (): void => done(new Error('Mock LLM exited before ready'))
    const failed = (error: Error): void => done(error)
    child.once('exit', exited)
    child.once('error', failed)
    lines.on('line', (line) => {
      if (!line.startsWith(handshakePrefix)) return
      try { done(undefined, JSON.parse(line.slice(handshakePrefix.length)) as Record<string, unknown>) }
      catch { done(new Error('Mock LLM emitted an invalid handshake')) }
    })
  })
}

async function getJson<T>(url: string): Promise<T> {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`Mock LLM control request failed: ${response.status}`)
  return response.json() as Promise<T>
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
