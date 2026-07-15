import { describe, expect, it, vi } from 'vitest'
import { DisposableStore } from '../../../../base/common/lifecycle'
import { parseDesktopHandshakeLine } from '../../common/desktop-handshake'
import { LoopalDesktopHost } from './desktop-host'
import { type DesktopHostGeneration } from './desktop-host-generation'
import { initializeGeneration } from './desktop-host-startup'
import {
  FakeChild,
  alive,
  createHost,
  fakeRpc,
} from './desktop-host.test-fixtures.ts'

describe('LoopalDesktopHost startup timeouts', () => {
  it('disposes an RPC connection that resolves after connect timeout', async () => {
    const child = new FakeChild()
    const rpcFixture = fakeRpc()
    let resolveConnection!: (rpc: typeof rpcFixture.rpc) => void
    const connection = new Promise<typeof rpcFixture.rpc>((resolve) => {
      resolveConnection = resolve
    })
    const fixture = createHost(child, rpcFixture, {
      startupTimeoutMs: 5,
      connectRpc: vi.fn(() => connection),
    })
    const start = fixture.host.start()
    child.stdout.write(alive())

    await expect(start).rejects.toThrow('did not connect')
    expect(rpcFixture.dispose).not.toHaveBeenCalled()
    resolveConnection(rpcFixture.rpc)
    await vi.waitFor(() => expect(rpcFixture.dispose).toHaveBeenCalledOnce())
    expect(fixture.host.currentStatus).toBe('crashed')
  })

  it('times out a pending Hub registration and releases its generation', async () => {
    const child = new FakeChild()
    const rpcFixture = fakeRpc(async (method) => {
      if (method === 'hub/register') return new Promise(() => undefined)
      return { ok: true }
    })
    const fixture = createHost(child, rpcFixture, { startupTimeoutMs: 5 })
    const start = fixture.host.start()
    child.stdout.write(alive())

    await expect(start).rejects.toThrow('did not register')
    expect(child.kill).toHaveBeenCalledWith('SIGTERM')
    expect(rpcFixture.dispose).toHaveBeenCalledOnce()
    expect(fixture.host.currentStatus).toBe('crashed')
  })

  it('abandons a timed-out connection before outer Host cleanup', async () => {
    const handshake = parseDesktopHandshakeLine(alive().trim())
    if (!handshake || handshake.phase !== 'alive') throw new Error('invalid fixture')
    const rpcFixture = fakeRpc()
    let resolveConnection!: (rpc: typeof rpcFixture.rpc) => void
    const connection = new Promise<typeof rpcFixture.rpc>((resolve) => {
      resolveConnection = resolve
    })
    const generation = {
      command: 1,
      process: {
        alive: Promise.resolve(handshake),
        ready: new Promise(() => undefined),
      },
      subscriptions: new DisposableStore(),
      closing: false,
      exited: false,
    } as unknown as DesktopHostGeneration
    const owns = vi.fn(() => true)
    const initialization = initializeGeneration(
      generation,
      { binaryPath: '/bin/loopal', cwd: '/workspace', startupTimeoutMs: 5,
        connectRpc: vi.fn(() => connection) as never },
      { owns, assertOwned: vi.fn(), setStatus: vi.fn(), notify: vi.fn(), fail: vi.fn() },
      new Promise<never>(() => undefined),
    )

    await expect(initialization).rejects.toThrow('did not connect')
    resolveConnection(rpcFixture.rpc)
    await vi.waitFor(() => expect(rpcFixture.dispose).toHaveBeenCalledOnce())
    expect(owns()).toBe(true)
  })
})
