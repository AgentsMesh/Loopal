import { type SessionSummary } from './session-contracts'
import { canRestartSession, isSessionLive } from './session-lifecycle'

const base: SessionSummary = {
  id: 'session', workspaceId: 'workspace', title: 'Session', model: 'model', mode: 'agent',
  status: 'waiting', activeRuntimeId: 'runtime',
  createdAt: '2026-07-12T12:00:00.000Z', updatedAt: '2026-07-12T12:00:00.000Z',
}

describe('session lifecycle predicates', () => {
  it('requires both a live status and active runtime', () => {
    for (const status of ['starting', 'running', 'waiting', 'failed'] as const) {
      expect(isSessionLive({ ...base, status })).toBe(true)
    }
    const { activeRuntimeId: _runtime, ...inactive } = base
    expect(isSessionLive(inactive)).toBe(false)
    for (const status of ['stopped', 'archived'] as const) {
      expect(isSessionLive({ ...base, status })).toBe(false)
    }
  })

  it('only prevents archived sessions from restarting', () => {
    expect(canRestartSession({ ...base, status: 'failed' })).toBe(true)
    expect(canRestartSession({ ...base, status: 'stopped' })).toBe(true)
    expect(canRestartSession({ ...base, status: 'archived' })).toBe(false)
  })
})
