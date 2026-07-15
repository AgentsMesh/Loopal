import { type RuntimeSummary, type SessionSummary } from '../../../../shared/contracts'
import { resolveMetaHubRuntimeTarget } from './session-runtime-target'

const session: SessionSummary = {
  id: 'session', workspaceId: 'workspace', title: 'Session', model: 'gpt-5', mode: 'act',
  status: 'waiting', activeRuntimeId: 'runtime-2',
  createdAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
}
const runtime: RuntimeSummary = {
  id: 'runtime-2', sessionId: 'session', workspaceId: 'workspace', generation: 2,
  state: 'ready', rootAgent: 'main',
}

describe('resolveMetaHubRuntimeTarget', () => {
  it('resolves only the exact active ready runtime', () => {
    expect(resolveMetaHubRuntimeTarget('session', [session], [
      { ...runtime, id: 'runtime-1', generation: 1, state: 'stopped' }, runtime,
    ])).toEqual({ sessionId: 'session', runtimeId: 'runtime-2', generation: 2 })
  })

  it.each([
    ['missing session', [], [runtime]],
    ['missing active id', [{ ...session, activeRuntimeId: undefined }], [runtime]],
    ['stopped session', [{ ...session, status: 'stopped' }], [runtime]],
    ['missing projection', [session], []],
    ['wrong session', [session], [{ ...runtime, sessionId: 'other' }]],
    ['wrong workspace', [session], [{ ...runtime, workspaceId: 'other' }]],
    ['starting', [session], [{ ...runtime, state: 'starting' }]],
    ['stopping', [session], [{ ...runtime, state: 'stopping' }]],
    ['stopped', [session], [{ ...runtime, state: 'stopped' }]],
    ['crashed', [session], [{ ...runtime, state: 'crashed' }]],
  ] as const)('rejects %s', (_name, sessions, runtimes) => {
    expect(resolveMetaHubRuntimeTarget(
      'session', sessions as readonly SessionSummary[], runtimes as readonly RuntimeSummary[],
    )).toBeUndefined()
  })

  it('does not guess the newest generation during a restart transition', () => {
    expect(resolveMetaHubRuntimeTarget('session', [session], [
      { ...runtime, id: 'runtime-3', generation: 3 },
      { ...runtime, state: 'stopped' },
    ])).toBeUndefined()
  })
})
