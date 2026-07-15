import { describe, expect, it, vi } from 'vitest'
import { LoopalDesktopHost } from './desktop-host'
import { JsonRpcClient } from '../rpc/jsonrpc-client'
import {
  FakeChild,
  alive,
  createHost,
  fakeRpc,
  ready,
} from './desktop-host.test-fixtures.ts'

describe('LoopalDesktopHost defaults and operation cleanup', () => {
  it('uses the default JSON-RPC connector', async () => {
    const child = new FakeChild()
    const rpcFixture = fakeRpc()
    const connect = vi.spyOn(JsonRpcClient, 'connect').mockResolvedValue(rpcFixture.rpc)
    const fixture = createHost(child, rpcFixture, { connectRpc: undefined })
    try {
      const start = fixture.host.start()
      child.stdout.write(alive())
      child.stdout.write(ready())
      await start
      expect(connect).toHaveBeenCalledWith('127.0.0.1:4567')
      await fixture.host.stop()
    } finally {
      connect.mockRestore()
    }
  })

  it('uses the shell-free default process spawner', async () => {
    const host = new LoopalDesktopHost({
      binaryPath: process.execPath,
      cwd: process.cwd(),
      startupTimeoutMs: 5_000,
      shutdownTimeoutMs: 100,
    })
    await expect(host.start()).rejects.toThrow('exited before shutdown')
    expect(host.currentStatus).toBe('crashed')
    await host.stop()
  })

  it('clears a rejected stop operation before the next lifecycle command', async () => {
    const fixture = createHost()
    const start = fixture.host.start()
    const internals = fixture.host as unknown as {
      release: () => Promise<void>
    }
    const release = vi.spyOn(internals, 'release').mockRejectedValueOnce(new Error('cleanup failed'))

    await expect(fixture.host.stop()).rejects.toThrow('cleanup failed')
    release.mockRestore()
    fixture.child.exit(1)
    await expect(start).rejects.toThrow()
    await fixture.host.stop()
  })

  it('forwards a valid resume ID and rejects invalid IDs before spawning', async () => {
    const resumed = createHost(new FakeChild(), fakeRpc(), { resumeSessionId: 'session-1' })
    const start = resumed.host.start()
    expect(resumed.spawnProcess).toHaveBeenCalledWith(
      '/bin/loopal', '/workspace', 321, undefined, 'session-1',
    )
    resumed.child.exit(1)
    await expect(start).rejects.toThrow()

    expect(() => createHost(new FakeChild(), fakeRpc(), { resumeSessionId: '--parent-pid' }))
      .toThrow('Invalid Loopal Desktop resume session ID')
  })
})
