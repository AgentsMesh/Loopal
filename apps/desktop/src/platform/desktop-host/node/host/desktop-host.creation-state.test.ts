import { describe, expect, it, vi } from 'vitest'
import {
  FakeChild, alive, childPid, createHost, fakeRpc, parentPid, sessionCreated,
} from './desktop-host.test-fixtures.ts'

describe('LoopalDesktopHost creation-state safety', () => {
  it('keeps unknown state when a marker arrives after a terminal handshake', async () => {
    const fixture = createHost()
    const activate = vi.fn(async () => undefined)
    const start = fixture.host.start(activate)
    fixture.child.stdout.write(alive())
    fixture.child.stdout.write(startupError())
    fixture.child.stdout.write(sessionCreated())

    await expect(start).rejects.toThrow('desktop_session_creation_state_unknown')
    expect(activate).not.toHaveBeenCalled()
  })

  it('keeps a pre-ALIVE failure eligible for ordinary rollback', async () => {
    const fixture = createHost()
    const start = fixture.host.start()
    fixture.child.stdout.write(startupError())
    const error = await rejectedError(start)

    expect(error.message).toContain('startup_failed')
    expect(error.message).not.toContain('desktop_session_creation_state_unknown')
  })

  it('does not classify a resumed Host failure as fresh creation uncertainty', async () => {
    const fixture = createHost(new FakeChild(), fakeRpc(), {
      resumeSessionId: 'session-existing',
    })
    const start = fixture.host.start()
    fixture.child.stdout.write(alive())
    fixture.child.stdout.write(startupError())
    const error = await rejectedError(start)

    expect(error.message).toContain('startup_failed')
    expect(error.message).not.toContain('desktop_session_creation_state_unknown')
  })
})

function startupError(): string {
  return `LOOPAL_DESKTOP ${JSON.stringify({
    protocol_version: 1,
    server_version: '0.6.3',
    pid: childPid,
    parent_pid: parentPid,
    phase: 'error',
    code: 'startup_failed',
    message: 'agent start failed',
  })}\n`
}

async function rejectedError(promise: Promise<unknown>): Promise<Error> {
  try {
    await promise
  } catch (error) {
    if (error instanceof Error) return error
    throw new Error('expected an Error rejection')
  }
  throw new Error('expected rejection')
}
