import { describe, expect, it, vi } from 'vitest'
import { LoopalDesktopHost } from './desktop-host'
import {
  FakeChild,
  alive,
  childPid,
  createHost,
  fakeRpc,
  parentPid,
  ready,
  startReady,
} from './desktop-host.test-fixtures.ts'

describe('LoopalDesktopHost lifecycle', () => {
  it('performs the alive/register/ready state machine and forwards notifications', async () => {
    const fixture = await startReady()
    expect(fixture.session).toEqual({ sessionId: 'session-1', serverVersion: '0.6.3', pid: childPid })
    expect(fixture.statuses).toEqual(['spawning', 'alive', 'registering', 'ready'])
    expect(fixture.spawnProcess).toHaveBeenCalledWith(
      '/bin/loopal', '/workspace', 321, undefined, undefined,
    )
    expect(fixture.rpcFixture.call).toHaveBeenCalledWith('hub/register', {
      name: 'desktop-test',
      token: 'secret',
      role: 'ui_client',
    })

    const listener = vi.fn()
    fixture.host.onNotification(listener)
    fixture.rpcFixture.notifications.fire({ method: 'agent/event', params: { payload: 'Running' } })
    expect(listener).toHaveBeenCalledOnce()
    await expect(fixture.host.request('view/snapshot', { agent: 'main' })).resolves.toEqual({ ok: true })
    await expect(fixture.host.request('hub/control', {
      target: 'main', command: 'Clear',
    })).resolves.toEqual({ ok: true })
    expect(fixture.rpcFixture.call).toHaveBeenCalledWith('hub/control', {
      target: 'main', command: 'Clear',
    }, undefined)
    await expect(fixture.host.request('agent/control')).rejects.toThrow('not allowlisted')
    await expect(fixture.host.request('agent/interrupt')).rejects.toThrow('not allowlisted')
    await expect(fixture.host.request('hub/secret/get')).rejects.toThrow('not allowlisted')
    expect(await fixture.host.start()).toBe(fixture.session)
  })

  it('reuses one in-flight startup promise', async () => {
    const fixture = createHost()
    const first = fixture.host.start()
    const second = fixture.host.start()
    expect(second).toBe(first)
    fixture.child.stdout.write(alive())
    fixture.child.stdout.write(ready())
    await expect(first).resolves.toMatchObject({ sessionId: 'session-1' })
  })

  it('rejects requests before ready and stops without having started', async () => {
    const { host } = createHost()
    await expect(host.request('hub/list_agents')).rejects.toThrow('not ready')
    await expect(host.stop()).resolves.toBeUndefined()
    expect(host.currentStatus).toBe('stopped')
  })

  it('shuts down the Hub and observes the child exit', async () => {
    const child = new FakeChild()
    const rpcFixture = fakeRpc(async (method) => {
      if (method === 'hub/register') return { ok: true }
      if (method === 'hub/shutdown') {
        queueMicrotask(() => child.exit(0))
        return { ok: true }
      }
      return {}
    })
    const fixture = await startReady(createHost(child, rpcFixture))
    await fixture.host.stop()
    expect(rpcFixture.call).toHaveBeenCalledWith('hub/shutdown', {})
    expect(rpcFixture.dispose).toHaveBeenCalled()
    expect(fixture.host.currentStatus).toBe('stopped')
  })

  it('coalesces concurrent shutdown and ignores close events during intentional stop', async () => {
    const child = new FakeChild()
    let finishShutdown!: () => void
    const shutdown = new Promise<void>((resolve) => {
      finishShutdown = resolve
    })
    const rpcFixture = fakeRpc(async (method) => {
      if (method === 'hub/register') return { ok: true }
      if (method === 'hub/shutdown') {
        await shutdown
        queueMicrotask(() => child.exit(0))
        return { ok: true }
      }
      return {}
    })
    const fixture = await startReady(createHost(child, rpcFixture))
    const first = fixture.host.stop()
    const second = fixture.host.stop()
    expect(second).toBe(first)
    expect(fixture.host.currentStatus).toBe('stopping')
    rpcFixture.closed.fire(undefined)
    expect(fixture.host.currentStatus).toBe('stopping')
    finishShutdown()
    await first
  })

  it('falls back to TERM/KILL when graceful shutdown fails or stalls', async () => {
    const failingChild = new FakeChild()
    const failure = fakeRpc(async (method) => {
      if (method === 'hub/register') return { ok: true }
      throw new Error('socket failed')
    })
    const first = await startReady(createHost(failingChild, failure))
    await first.host.stop()
    expect(failingChild.kill).toHaveBeenCalledWith('SIGTERM')

    const stalledChild = new FakeChild()
    stalledChild.autoExitOnKill = false
    const second = await startReady(createHost(stalledChild))
    stalledChild.stderr.write('stalled diagnostic\n')
    await vi.waitFor(() => expect(second.host.diagnostics).toContain('stalled diagnostic'))
    const stop = second.host.stop()
    await vi.waitFor(() => expect(stalledChild.kill).toHaveBeenCalledWith('SIGKILL'))
    expect(second.rpcFixture.dispose).not.toHaveBeenCalled()
    stalledChild.exit(null, 'SIGKILL')
    await stop
    expect(stalledChild.kill).toHaveBeenCalledWith('SIGKILL')
    expect(second.host.diagnostics).toContain('stalled diagnostic')
  })

  it('keeps restart blocked until the killed generation is reaped', async () => {
    const firstChild = new FakeChild()
    firstChild.autoExitOnKill = false
    const secondChild = new FakeChild()
    const firstRpc = fakeRpc()
    const secondRpc = fakeRpc()
    let childIndex = 0
    let rpcIndex = 0
    const spawnProcess = vi.fn(() => [firstChild, secondChild][childIndex++]!)
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
    firstRpc.closed.fire(new Error('socket lost'))

    const restart = host.start()
    await vi.waitFor(() => expect(firstChild.kill).toHaveBeenCalledWith('SIGKILL'))
    expect(spawnProcess).toHaveBeenCalledOnce()
    expect(firstRpc.dispose).not.toHaveBeenCalled()
    firstChild.exit(null, 'SIGKILL')
    await vi.waitFor(() => expect(spawnProcess).toHaveBeenCalledTimes(2))
    secondChild.stdout.write(alive())
    secondChild.stdout.write(ready({ session_id: 'session-2' }))
    await restart
    expect(firstRpc.dispose).toHaveBeenCalledOnce()
    await host.stop()
  })
})
