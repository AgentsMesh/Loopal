import { describe, expect, it, vi } from 'vitest'
import { LoopalDesktopHost } from './desktop-host'
import {
  FakeChild,
  alive,
  fakeRpc,
  parentPid,
  ready,
} from './desktop-host.test-fixtures.ts'

function createSequence(
  children: FakeChild[],
  rpcFixtures: ReturnType<typeof fakeRpc>[],
  shutdownTimeoutMs = 5,
) {
  let childIndex = 0
  let rpcIndex = 0
  const spawnProcess = vi.fn(() => children[childIndex++]!)
  const connectRpc = vi.fn(async () => rpcFixtures[rpcIndex++]!.rpc)
  const host = new LoopalDesktopHost({
    binaryPath: '/bin/loopal',
    cwd: '/workspace',
    parentPid,
    startupTimeoutMs: 50,
    shutdownTimeoutMs,
    clientName: 'desktop-test',
    spawnProcess: spawnProcess as never,
    connectRpc: connectRpc as never,
  })
  return { host, spawnProcess, connectRpc }
}

async function finishStart(
  child: FakeChild,
  start: Promise<unknown>,
  sessionId = 'session-1',
): Promise<void> {
  child.stdout.write(alive())
  child.stdout.write(ready({ session_id: sessionId }))
  await start
}

describe('LoopalDesktopHost generations', () => {
  it('restarts after reaping RPC-loss generation and detaches its callbacks', async () => {
    const firstChild = new FakeChild()
    firstChild.autoExitOnKill = false
    const secondChild = new FakeChild()
    const firstRpc = fakeRpc()
    const secondRpc = fakeRpc()
    const fixture = createSequence([firstChild, secondChild], [firstRpc, secondRpc])
    const notification = vi.fn()
    fixture.host.onNotification(notification)
    await finishStart(firstChild, fixture.host.start())
    const firstGeneration = (fixture.host as unknown as { active: unknown }).active

    firstRpc.closed.fire(new Error('socket lost'))
    expect(fixture.host.currentStatus).toBe('crashed')
    await expect(fixture.host.request('view/snapshot')).rejects.toThrow('not ready')
    expect(firstChild.kill).toHaveBeenCalledWith('SIGTERM')

    const restart = fixture.host.start()
    await vi.waitFor(() => expect(firstChild.kill).toHaveBeenCalledWith('SIGKILL'))
    expect(fixture.spawnProcess).toHaveBeenCalledOnce()
    expect(firstRpc.dispose).not.toHaveBeenCalled()
    firstChild.exit(null, 'SIGKILL')
    await vi.waitFor(() => expect(fixture.spawnProcess).toHaveBeenCalledTimes(2))
    await finishStart(secondChild, restart, 'session-2')
    firstRpc.notifications.fire({ method: 'old/event', params: {} })
    firstRpc.closed.fire(new Error('late close'))
    const internals = fixture.host as unknown as {
      failGeneration: (generation: unknown, error: Error) => void
    }
    internals.failGeneration(firstGeneration, new Error('late callback'))

    expect(notification).not.toHaveBeenCalled()
    expect(fixture.host.currentStatus).toBe('ready')
    await expect(fixture.host.request('hub/list_agents')).resolves.toEqual({ ok: true })
    expect(secondRpc.call).toHaveBeenCalledWith('hub/list_agents', {}, undefined)
    expect(firstRpc.dispose).toHaveBeenCalledOnce()
    await fixture.host.stop()
  })

  it('waits for TERM exit and disposes RPC after startup failure', async () => {
    const child = new FakeChild()
    child.autoExitOnKill = false
    const rpcFixture = fakeRpc(async () => ({ ok: false }))
    const fixture = createSequence([child], [rpcFixture], 500)
    const start = fixture.host.start()
    let settled = false
    void start.then(
      () => { settled = true },
      () => { settled = true },
    )
    child.stdout.write(alive())

    await vi.waitFor(() => expect(child.kill).toHaveBeenCalledWith('SIGTERM'))
    await Promise.resolve()
    expect(settled).toBe(false)
    expect(rpcFixture.dispose).not.toHaveBeenCalled()
    child.exit(null, 'SIGTERM')
    await expect(start).rejects.toThrow('rejected')
    expect(rpcFixture.dispose).toHaveBeenCalledOnce()
    expect(child.kill).not.toHaveBeenCalledWith('SIGKILL')
    expect(fixture.host.currentStatus).toBe('crashed')
  })

  it('serializes stop against startup and coalesces the queued restart', async () => {
    const firstChild = new FakeChild()
    const secondChild = new FakeChild()
    const fixture = createSequence(
      [firstChild, secondChild],
      [fakeRpc(), fakeRpc()],
    )
    const firstStart = fixture.host.start()
    const firstStop = fixture.host.stop()
    const secondStop = fixture.host.stop()
    expect(secondStop).toBe(firstStop)
    const firstRestart = fixture.host.start()
    const secondRestart = fixture.host.start()
    expect(secondRestart).toBe(firstRestart)

    await firstStop
    await expect(firstStart).rejects.toThrow('exited before shutdown')
    await vi.waitFor(() => expect(fixture.spawnProcess).toHaveBeenCalledTimes(2))
    await finishStart(secondChild, firstRestart, 'session-2')
    expect(fixture.host.currentStatus).toBe('ready')
    await fixture.host.stop()
  })

  it('disposes a stale RPC connection without touching its replacement', async () => {
    const firstChild = new FakeChild()
    const secondChild = new FakeChild()
    const firstRpc = fakeRpc()
    const secondRpc = fakeRpc()
    let resolveFirst!: () => void
    const firstConnection = new Promise<void>((resolve) => { resolveFirst = resolve })
    let calls = 0
    const spawnProcess = vi.fn(() => [firstChild, secondChild][calls]!)
    const connectRpc = vi.fn(async () => {
      const call = ++calls
      if (call === 1) await firstConnection
      return call === 1 ? firstRpc.rpc : secondRpc.rpc
    })
    const host = new LoopalDesktopHost({
      binaryPath: '/bin/loopal', cwd: '/workspace', parentPid,
      startupTimeoutMs: 50, shutdownTimeoutMs: 5,
      spawnProcess: spawnProcess as never, connectRpc: connectRpc as never,
    })
    const staleStart = host.start()
    firstChild.stdout.write(alive())
    await vi.waitFor(() => expect(connectRpc).toHaveBeenCalledOnce())
    await host.stop()
    const currentStart = host.start()
    await vi.waitFor(() => expect(spawnProcess).toHaveBeenCalledTimes(2))
    secondChild.stdout.write(alive())
    secondChild.stdout.write(ready({ session_id: 'session-2' }))
    await currentStart
    resolveFirst()
    await expect(staleStart).rejects.toThrow('superseded')
    expect(firstRpc.dispose).toHaveBeenCalledOnce()
    expect(host.currentStatus).toBe('ready')
    await host.stop()
  })

  it('cancels a restart superseded while crash cleanup is pending', async () => {
    const child = new FakeChild()
    child.autoExitOnKill = false
    const rpcFixture = fakeRpc()
    const fixture = createSequence([child], [rpcFixture])
    await finishStart(child, fixture.host.start())
    rpcFixture.closed.fire(new Error('socket lost'))

    const restart = fixture.host.start()
    const stop = fixture.host.stop()
    await vi.waitFor(() => expect(child.kill).toHaveBeenCalledWith('SIGKILL'))
    child.exit(null, 'SIGKILL')
    await expect(restart).rejects.toThrow('superseded')
    await stop
    expect(fixture.spawnProcess).toHaveBeenCalledOnce()
    expect(fixture.host.currentStatus).toBe('stopped')
  })
})
