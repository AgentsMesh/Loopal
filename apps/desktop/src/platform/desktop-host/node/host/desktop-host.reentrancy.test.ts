import { describe, expect, it, vi } from 'vitest'
import { LoopalDesktopHost } from './desktop-host'
import {
  FakeChild,
  alive,
  createHost,
  fakeRpc,
  parentPid,
  ready,
} from './desktop-host.test-fixtures.ts'

describe('LoopalDesktopHost lifecycle reentrancy', () => {
  it('publishes the startup operation before spawning status fires', async () => {
    const fixture = createHost()
    let nestedStart: Promise<unknown> | undefined
    fixture.host.onStatus((status) => {
      if (status === 'spawning') nestedStart = fixture.host.start()
    })
    const start = fixture.host.start()
    expect(nestedStart).toBe(start)
    fixture.child.stdout.write(alive())
    fixture.child.stdout.write(ready())
    await start
    expect(fixture.spawnProcess).toHaveBeenCalledOnce()
    await fixture.host.stop()
  })

  it('publishes stop before a status listener requests restart', async () => {
    const firstChild = new FakeChild()
    const secondChild = new FakeChild()
    const firstRpc = fakeRpc(async (method) => {
      if (method === 'hub/shutdown') {
        queueMicrotask(() => firstChild.exit(0))
      }
      return { ok: true }
    })
    const secondRpc = fakeRpc()
    let spawnIndex = 0
    let rpcIndex = 0
    const spawnProcess = vi.fn(() => [firstChild, secondChild][spawnIndex++]!)
    const connectRpc = vi.fn(async () => [firstRpc.rpc, secondRpc.rpc][rpcIndex++]!)
    const host = new LoopalDesktopHost({
      binaryPath: '/bin/loopal', cwd: '/workspace', parentPid,
      startupTimeoutMs: 50, shutdownTimeoutMs: 5,
      spawnProcess: spawnProcess as never, connectRpc: connectRpc as never,
    })
    const firstStart = host.start()
    firstChild.stdout.write(alive())
    firstChild.stdout.write(ready())
    await firstStart
    let restart: Promise<unknown> | undefined
    host.onStatus((status) => {
      if (status === 'stopping') restart = host.start()
    })

    const stop = host.stop()
    expect(restart).toBeDefined()
    await stop
    await vi.waitFor(() => expect(spawnProcess).toHaveBeenCalledTimes(2))
    secondChild.stdout.write(alive())
    secondChild.stdout.write(ready({ session_id: 'session-2' }))
    await restart
    expect(host.currentStatus).toBe('ready')
    await host.stop()
  })
})
