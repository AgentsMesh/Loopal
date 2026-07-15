import { describe, expect, it, vi } from 'vitest'
import { LoopalDesktopHost, spawnDesktopProcess } from './desktop-host'
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

describe('LoopalDesktopHost failures and process invocation', () => {
  it.each([
    ['malformed JSON', 'LOOPAL_DESKTOP {broken}\n', 'JSON'],
    ['wrong pid', alive({ pid: 999 }), 'PID metadata'],
    ['wrong parent pid', alive({ parent_pid: 999 }), 'PID metadata'],
    [
      'structured error',
      `LOOPAL_DESKTOP ${JSON.stringify({
        protocol_version: 1,
        server_version: '0.6.3',
        pid: childPid,
        parent_pid: parentPid,
        phase: 'error',
        code: 'startup_failed',
        message: 'could not start',
      })}\n`,
      'startup_failed',
    ],
  ])('rejects a %s handshake', async (_name, line, expected) => {
    const fixture = createHost()
    const start = fixture.host.start()
    fixture.child.stderr.write('diagnostic line\n')
    fixture.child.stdout.write(line)
    await expect(start).rejects.toThrow(expected)
    await expect(start).rejects.toThrow('diagnostic line')
    expect(fixture.child.kill).toHaveBeenCalledWith('SIGTERM')
  })

  it('rejects registration failure, early process exit, and startup timeout', async () => {
    const rejected = createHost(new FakeChild(), fakeRpc(async () => ({ ok: false })))
    const registration = rejected.host.start()
    rejected.child.stdout.write(alive())
    await expect(registration).rejects.toThrow('rejected')

    const malformedRegistration = createHost(new FakeChild(), fakeRpc(async () => 'invalid'))
    const malformed = malformedRegistration.host.start()
    malformedRegistration.child.stdout.write(alive())
    await expect(malformed).rejects.toThrow('rejected')

    for (const registrationValue of [null, []]) {
      const invalid = createHost(new FakeChild(), fakeRpc(async () => registrationValue))
      const invalidStart = invalid.host.start()
      invalid.child.stdout.write(alive())
      await expect(invalidStart).rejects.toThrow('rejected')
    }

    const nonError = createHost(new FakeChild(), fakeRpc(), {
      connectRpc: vi.fn(async () => Promise.reject('connect failed')),
    })
    const nonErrorStart = nonError.host.start()
    nonError.child.stdout.write(alive())
    await expect(nonErrorStart).rejects.toThrow('connect failed')

    const early = createHost()
    const start = early.host.start()
    early.child.exit(7, null)
    await expect(start).rejects.toThrow('code=7')
    expect(early.host.currentStatus).toBe('crashed')

    const timedOut = createHost(new FakeChild(), fakeRpc(), { startupTimeoutMs: 1 })
    await expect(timedOut.host.start()).rejects.toThrow('did not emit alive')

    const readyTimedOut = createHost(new FakeChild(), fakeRpc(), { startupTimeoutMs: 1 })
    const waitingForReady = readyTimedOut.host.start()
    readyTimedOut.child.stdout.write(alive())
    await expect(waitingForReady).rejects.toThrow('did not create or report a Session')
  })

  it('rejects a server version change between alive and ready', async () => {
    const fixture = createHost()
    const start = fixture.host.start()
    fixture.child.stdout.write(alive())
    await vi.waitFor(() => expect(fixture.connectRpc).toHaveBeenCalled())
    fixture.child.stdout.write(ready({ server_version: '9.9.9' }))
    await expect(start).rejects.toThrow('changed server version')
  })

  it('reports a child-process spawn error and ignores its later exit', async () => {
    const fixture = createHost()
    const start = fixture.host.start()
    fixture.child.fail(new Error('spawn failed'))
    await expect(start).rejects.toThrow('spawn failed')
    await Promise.resolve()
    expect(fixture.host.currentStatus).toBe('crashed')
  })

  it('uses process defaults and stops a child before the RPC connection exists', async () => {
    const child = new FakeChild()
    const spawnProcess = vi.fn(() => child)
    const host = new LoopalDesktopHost({
      binaryPath: '/bin/loopal',
      cwd: '/workspace',
      spawnProcess: spawnProcess as never,
    })
    const start = host.start()
    const stop = host.stop()
    await expect(stop).resolves.toBeUndefined()
    await expect(start).rejects.toThrow('exited before shutdown')
    expect(spawnProcess).toHaveBeenCalledWith(
      '/bin/loopal', '/workspace', process.pid, undefined, undefined,
    )
  })

  it('generates a unique default UI client name', async () => {
    const fixture = createHost(new FakeChild(), fakeRpc(), { clientName: undefined })
    const start = fixture.host.start()
    fixture.child.stdout.write(alive())
    fixture.child.stdout.write(ready())
    await start
    expect(fixture.rpcFixture.call).toHaveBeenCalledWith(
      'hub/register',
      expect.objectContaining({ name: expect.stringMatching(/^loopal-desktop-[0-9a-f-]{36}$/) }),
    )
  })

  it('stages the exact shell-free child-process invocation', () => {
    const child = new FakeChild()
    const spawnFn = vi.fn(() => child)
    expect(
      spawnDesktopProcess(
        '/runtime/loopal', '/project', 88, { CUSTOM: 'yes' }, undefined, spawnFn as never,
      ),
    ).toBe(child)
    expect(spawnFn).toHaveBeenCalledWith(
      '/runtime/loopal',
      ['desktop', 'serve', '--parent-pid', '88'],
      expect.objectContaining({
        cwd: '/project',
        env: expect.objectContaining({ CUSTOM: 'yes' }),
        shell: false,
        windowsHide: true,
        stdio: ['ignore', 'pipe', 'pipe'],
      }),
    )
  })

  it('passes a validated resume ID as one shell-free argv value', () => {
    const child = new FakeChild()
    const spawnFn = vi.fn(() => child)
    expect(spawnDesktopProcess(
      '/runtime/loopal', '/project', 88, undefined, 'session-1', spawnFn as never,
    )).toBe(child)
    expect(spawnFn).toHaveBeenCalledWith(
      '/runtime/loopal',
      ['desktop', 'serve', '--parent-pid', '88', '--resume', 'session-1'],
      expect.objectContaining({ shell: false }),
    )
  })

  it.each(['--parent-pid', 'session;touch-pwned', 'session/name', 'session\nnext'])(
    'rejects resume argv injection %j before spawning',
    (sessionId) => {
      const spawnFn = vi.fn(() => new FakeChild())
      expect(() => spawnDesktopProcess(
        '/runtime/loopal', '/project', 88, undefined, sessionId, spawnFn as never,
      )).toThrow('Invalid Loopal Desktop resume session ID')
      expect(spawnFn).not.toHaveBeenCalled()
    },
  )

  it('marks an established connection crash and caps diagnostic history', async () => {
    const fixture = await startReady()
    for (let index = 0; index < 110; index += 1) {
      fixture.child.stderr.write(`${index}-${'x'.repeat(2_100)}\n`)
    }
    await vi.waitFor(() => expect(fixture.host.diagnostics).toHaveLength(100))
    expect(fixture.host.diagnostics[0]).toMatch(/^10-/)
    expect(fixture.host.diagnostics.at(-1)?.length).toBe(2_000)
    fixture.rpcFixture.closed.fire(new Error('connection lost'))
    expect(fixture.host.currentStatus).toBe('crashed')
    fixture.host.dispose()
  })
})
