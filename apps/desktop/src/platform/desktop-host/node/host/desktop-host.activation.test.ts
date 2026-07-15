import { describe, expect, it, vi } from 'vitest'
import { DeferredPromise } from '../../../../base/common/async'
import { LoopalDesktopHost } from './desktop-host'
import {
  FakeChild, alive, createHost, fakeRpc, parentPid, ready, sessionCreated,
} from './desktop-host.test-fixtures.ts'

describe('LoopalDesktopHost session activation', () => {
  it('awaits activation before accepting READY', async () => {
    const fixture = createHost()
    const gate = new DeferredPromise<void>()
    const activate = vi.fn(async () => gate.promise)
    const start = fixture.host.start(activate)
    fixture.child.stdout.write(alive())
    fixture.child.stdout.write(sessionCreated())
    fixture.child.stdout.write(ready())

    await vi.waitFor(() => expect(activate).toHaveBeenCalledOnce())
    expect(fixture.host.currentStatus).not.toBe('ready')
    gate.resolve(undefined)
    await expect(start).resolves.toMatchObject({ sessionId: 'session-1' })
    expect(fixture.host.currentStatus).toBe('ready')
  })

  it('uses READY as an exactly-once fallback for an old Host', async () => {
    const fixture = createHost()
    const activate = vi.fn(async () => undefined)
    const start = fixture.host.start(activate)
    fixture.child.stdout.write(alive())
    fixture.child.stdout.write(ready())

    await expect(start).resolves.toMatchObject({ sessionId: 'session-1' })
    expect(activate).toHaveBeenCalledOnce()
  })

  it('settles an observed marker before reporting registration failure', async () => {
    const fixture = createHost(new FakeChild(), fakeRpc(async () => ({ ok: false })))
    const activate = vi.fn(async () => undefined)
    const start = fixture.host.start(activate)
    fixture.child.stdout.write(alive())
    fixture.child.stdout.write(sessionCreated())

    await expect(start).rejects.toThrow('rejected')
    expect(activate).toHaveBeenCalledOnce()
  })

  it('marks a fresh ALIVE-without-identity failure as creation-state unknown', async () => {
    const fixture = createHost(new FakeChild(), fakeRpc(async () => ({ ok: false })))
    const start = fixture.host.start()
    fixture.child.stdout.write(alive())

    await expect(start).rejects.toThrow('desktop_session_creation_state_unknown')
  })

  it('drains a marker written after child exit and activation timeout', async () => {
    const child = new FakeChild()
    child.endStreamsOnExit = false
    const fixture = createHost(child, fakeRpc(), {
      startupTimeoutMs: 10, shutdownTimeoutMs: 5,
    })
    const activate = vi.fn(async () => undefined)
    const start = fixture.host.start(activate)
    child.stdout.write(alive())
    child.exit(7)
    setTimeout(() => {
      child.stdout.write(sessionCreated())
      child.stdout.end()
      child.stderr.end()
      child.emit('close', 7, null)
    }, 30)

    await expect(start).rejects.toThrow()
    expect(activate).toHaveBeenCalledOnce()
  })

  it('rejects conflicting marker IDs while preserving the first activation', async () => {
    const fixture = createHost()
    const activate = vi.fn(async () => undefined)
    const start = fixture.host.start(activate)
    fixture.child.stdout.write(alive())
    fixture.child.stdout.write(sessionCreated())
    fixture.child.stdout.write(sessionCreated({ session_id: 'session-2' }))
    fixture.child.stdout.write(ready())

    await expect(start).rejects.toThrow('changed Session')
    expect(activate).toHaveBeenCalledOnce()
    expect(activate).toHaveBeenCalledWith(expect.objectContaining({ sessionId: 'session-1' }))
  })

  it('bounds a post-SIGKILL stdio drain when a descendant holds the pipe', async () => {
    const child = new FakeChild()
    child.autoExitOnKill = false
    child.endStreamsOnExit = false
    child.kill.mockImplementation((signal) => {
      if (signal === 'SIGKILL') child.exit(null, signal)
      return true
    })
    const fixture = createHost(child)
    const start = fixture.host.start()
    child.stdout.write(alive())
    child.stdout.write(ready())
    await start

    await expect(fixture.host.stop()).rejects.toThrow('desktop_protocol_drain_incomplete')
    expect(child.kill).toHaveBeenCalledWith('SIGKILL')
    expect(child.stdout.destroyed).toBe(true)
    expect(child.stderr.destroyed).toBe(true)
  })

  it('does not start a replacement while process termination is unconfirmed', async () => {
    const child = new FakeChild()
    child.autoExitOnKill = false
    let emittedKillError = false
    child.kill.mockImplementation((signal) => {
      if (signal === 'SIGKILL' && !emittedKillError) {
        emittedKillError = true
        child.emit('error', new Error('kill failed'))
      }
      return true
    })
    const fixture = createHost(child)
    const start = fixture.host.start()
    child.stdout.write(alive())
    child.stdout.write(ready())
    await start

    await expect(fixture.host.stop()).rejects.toThrow(
      'desktop_process_termination_unconfirmed',
    )
    await expect(fixture.host.start()).rejects.toThrow(
      'desktop_process_termination_unconfirmed',
    )
    expect(fixture.spawnProcess).toHaveBeenCalledOnce()
    child.exit(null, 'SIGKILL')
    await expect(fixture.host.stop()).resolves.toBeUndefined()
  })

  it('preserves a late drain error after the activation timeout settled', async () => {
    const child = new FakeChild()
    child.autoExitOnKill = false
    child.endStreamsOnExit = false
    child.kill.mockImplementation((signal) => {
      if (signal === 'SIGKILL') child.exit(null, signal)
      return true
    })
    const fixture = createHost(child, fakeRpc(), {
      startupTimeoutMs: 10, shutdownTimeoutMs: 5,
    })
    const start = fixture.host.start(vi.fn(async () => undefined))
    child.stdout.write(alive())

    await expect(start).rejects.toThrow('desktop_protocol_drain_incomplete')
    expect(child.stdout.destroyed).toBe(true)
  })

  it('settles activation despite cleanup failure and permits a new generation', async () => {
    const children = [new FakeChild(), new FakeChild()]
    const firstRpc = fakeRpc(async () => ({ ok: false }))
    firstRpc.dispose.mockImplementationOnce(() => { throw new Error('dispose failed') })
    const secondRpc = fakeRpc()
    let childIndex = 0
    let rpcIndex = 0
    const host = new LoopalDesktopHost({
      binaryPath: '/bin/loopal', cwd: '/workspace', parentPid,
      startupTimeoutMs: 1_000, shutdownTimeoutMs: 5,
      spawnProcess: vi.fn(() => children[childIndex++]!) as never,
      connectRpc: vi.fn(async () => [firstRpc.rpc, secondRpc.rpc][rpcIndex++]!) as never,
    })
    const gate = new DeferredPromise<void>()
    const activate = vi.fn(async () => gate.promise)
    const first = host.start(activate)
    children[0]!.stdout.write(alive())
    children[0]!.stdout.write(sessionCreated())
    await vi.waitFor(() => expect(activate).toHaveBeenCalledOnce())
    let rejected = false
    void first.catch(() => { rejected = true })
    await Promise.resolve()
    expect(rejected).toBe(false)
    gate.resolve(undefined)
    await expect(first).rejects.toThrow('cleanup failed (dispose failed)')

    const second = host.start()
    children[1]!.stdout.write(alive())
    children[1]!.stdout.write(ready({ session_id: 'session-2' }))
    await expect(second).resolves.toMatchObject({ sessionId: 'session-2' })
  })
})
