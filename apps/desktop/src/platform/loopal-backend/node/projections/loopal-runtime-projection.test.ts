import { describe, expect, it } from 'vitest'
import { type SessionSummary } from '../../../../shared/contracts'
import {
  hostSession,
  runtimeFields,
  runtimeState,
  runtimeSummary,
} from './loopal-runtime-projection'

const now = new Date('2026-07-11T12:00:00.000Z')
const scope = {
  workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime', generation: 2,
}
const session: SessionSummary = {
  id: 'session', workspaceId: 'workspace', title: 'Session', model: 'model', mode: 'agent',
  status: 'running', activeRuntimeId: 'old', attention: 'permission',
  createdAt: now.toISOString(), updatedAt: now.toISOString(),
}

describe('Loopal runtime projection', () => {
  it('maps every Host status and preserves an existing start time', () => {
    expect(runtimeSummary(scope, 'ready', now)).toMatchObject({
      id: 'runtime', state: 'ready', startedAt: now.toISOString(),
    })
    expect(runtimeSummary(scope, 'stopping', now, '2026-07-11T10:00:00.000Z'))
      .toMatchObject({ state: 'stopping', startedAt: '2026-07-11T10:00:00.000Z' })
    expect([
      runtimeState('stopped'), runtimeState('crashed'), runtimeState('spawning'),
      runtimeState('alive'), runtimeState('registering'),
    ]).toEqual(['stopped', 'crashed', 'starting', 'starting', 'starting'])
  })

  it('clears stale runtime attention for normal transitions and marks crashes', () => {
    const statusEvent = (status: 'ready' | 'spawning' | 'stopping' | 'stopped' | 'crashed') => ({
      ...scope, status,
    })
    expect(hostSession(session, statusEvent('ready'), now.toISOString())).toMatchObject({
      status: 'waiting', activeRuntimeId: 'runtime',
    })
    expect(hostSession(session, statusEvent('spawning'), now.toISOString())).toMatchObject({
      status: 'starting', activeRuntimeId: 'runtime',
    })
    for (const status of ['stopping', 'stopped'] as const) {
      expect(hostSession(session, statusEvent(status), now.toISOString())).toEqual(
        expect.not.objectContaining({ activeRuntimeId: expect.anything(), attention: expect.anything() }),
      )
    }
    expect(hostSession(session, statusEvent('crashed'), now.toISOString())).toMatchObject({
      status: 'failed', attention: 'failure',
    })
    expect(runtimeFields(session)).toEqual({
      status: 'running', activeRuntimeId: 'old', attention: 'permission',
    })
    const { attention: _attention, ...plain } = session
    expect(runtimeFields(plain)).toEqual({ status: 'running', activeRuntimeId: 'old' })
  })
})
