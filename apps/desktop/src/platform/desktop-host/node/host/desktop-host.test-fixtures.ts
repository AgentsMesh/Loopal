import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'
import { expect, vi } from 'vitest'
import { Emitter } from '../../../../base/common/event'
import { LoopalDesktopHost } from './desktop-host'
import { type JsonRpcClient, type JsonRpcNotification } from '../rpc/jsonrpc-client'
import { DESKTOP_REQUIRED_CAPABILITIES } from '../../common/desktop-handshake'

export const parentPid = 321
export const childPid = 654

export class FakeChild extends EventEmitter {
  readonly pid = childPid
  readonly stdout = new PassThrough()
  readonly stderr = new PassThrough()
  exitCode: number | null = null
  signalCode: NodeJS.Signals | null = null
  autoExitOnKill = true
  endStreamsOnExit = true
  readonly kill = vi.fn((signal: NodeJS.Signals = 'SIGTERM') => {
    if (this.autoExitOnKill) {
      queueMicrotask(() => this.exit(null, signal))
    }
    return true
  })

  exit(code: number | null, signal: NodeJS.Signals | null = null): void {
    this.exitCode = code
    this.signalCode = signal
    this.emit('exit', code, signal)
    if (this.endStreamsOnExit) {
      this.stdout.end()
      this.stderr.end()
      queueMicrotask(() => this.emit('close', code, signal))
    }
  }

  fail(error: Error): void {
    this.emit('error', error)
    this.stdout.end()
    this.stderr.end()
    queueMicrotask(() => this.emit('close', null, null))
  }
}

export function alive(overrides: Record<string, unknown> = {}): string {
  return `LOOPAL_DESKTOP ${JSON.stringify({
    protocol_version: 1,
    server_version: '0.6.3',
    pid: childPid,
    parent_pid: parentPid,
    phase: 'alive',
    addr: '127.0.0.1:4567',
    token: 'secret',
    transport: 'tcp_jsonrpc_ndjson',
    capabilities: [...DESKTOP_REQUIRED_CAPABILITIES],
    ...overrides,
  })}\n`
}

export function ready(overrides: Record<string, unknown> = {}): string {
  return `LOOPAL_DESKTOP ${JSON.stringify({
    protocol_version: 1,
    server_version: '0.6.3',
    pid: childPid,
    parent_pid: parentPid,
    phase: 'ready',
    session_id: 'session-1',
    ...overrides,
  })}\n`
}

export function sessionCreated(overrides: Record<string, unknown> = {}): string {
  return `LOOPAL_DESKTOP_EVENT ${JSON.stringify({
    protocol_version: 1,
    server_version: '0.6.3',
    pid: childPid,
    parent_pid: parentPid,
    phase: 'session_created',
    session_id: 'session-1',
    ...overrides,
  })}\n`
}

export function fakeRpc(
  callImplementation?: (method: string, params: unknown) => Promise<unknown>,
) {
  const notifications = new Emitter<JsonRpcNotification>()
  const closed = new Emitter<Error | undefined>()
  const call = vi.fn(
    callImplementation ??
      (async (method: string) => (method === 'hub/register' ? { ok: true } : { ok: true })),
  )
  const dispose = vi.fn()
  const rpc = {
    call,
    dispose,
    onNotification: notifications.event,
    onClose: closed.event,
  } as unknown as JsonRpcClient
  return { rpc, call, dispose, notifications, closed }
}

export function createHost(
  child = new FakeChild(),
  rpcFixture = fakeRpc(),
  overrides: Record<string, unknown> = {},
) {
  const spawnProcess = vi.fn(() => child)
  const connectRpc = vi.fn(async () => rpcFixture.rpc)
  const host = new LoopalDesktopHost({
    binaryPath: '/bin/loopal',
    cwd: '/workspace',
    parentPid,
    startupTimeoutMs: 1_000,
    shutdownTimeoutMs: 5,
    clientName: 'desktop-test',
    spawnProcess: spawnProcess as never,
    connectRpc: connectRpc as never,
    ...overrides,
  })
  return { host, child, rpcFixture, spawnProcess, connectRpc }
}

export async function startReady(fixture = createHost()) {
  const statuses: string[] = []
  fixture.host.onStatus((status) => statuses.push(status))
  const start = fixture.host.start()
  fixture.child.stdout.write('ordinary diagnostic output\n')
  fixture.child.stdout.write(alive())
  await vi.waitFor(() => expect(fixture.connectRpc).toHaveBeenCalledWith('127.0.0.1:4567'))
  fixture.child.stdout.write(ready())
  const session = await start
  return { ...fixture, statuses, session }
}
