import { describe, expect, it } from 'vitest'
import { fallbackSession, stoppedSession } from './loopal-session-catalog'

describe('Loopal session catalog projection', () => {
  it('preserves persisted titles and uses the workspace only for fresh fallback', () => {
    const persisted = stoppedSession({
      id: 'session', title: 'Custom investigation', model: 'model', mode: 'agent',
      createdAt: '2026-07-11T10:00:00.000Z', updatedAt: '2026-07-11T11:00:00.000Z',
    }, 'workspace')
    expect(persisted.title).toBe('Custom investigation')
    expect(fallbackSession(
      'opaque-session-id', 'workspace', 'project', '2026-07-11T12:00:00.000Z',
    ).title).toBe('Loopal session · project')
  })
})
