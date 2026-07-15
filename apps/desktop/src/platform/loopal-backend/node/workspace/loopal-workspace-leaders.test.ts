import { describe, expect, it } from 'vitest'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'
import { LoopalWorkspaceLeaders } from './loopal-workspace-leaders'

function runtime(id: string, workspaceId = 'workspace'): SessionRuntimeHandle {
  return {
    workspaceId, sessionId: `session-${id}`, runtimeId: id, generation: 1,
    host: { currentStatus: 'ready' } as DesktopHostClient,
  }
}

describe('LoopalWorkspaceLeaders', () => {
  it('handles leader, follower, retiring, direct terminal, and missing transitions', () => {
    const leaders = new LoopalWorkspaceLeaders()
    const first = runtime('first')
    const second = runtime('second')
    expect(leaders.current('missing')).toBeUndefined()
    expect(leaders.add(first)).toBe(true)
    expect(leaders.add(second)).toBe(false)
    expect(leaders.transition('second', 'workspace', 'stopping')).toEqual([])
    expect(leaders.transition('missing', 'workspace', 'ready')).toEqual([])
    expect(leaders.transition('first', 'workspace', 'ready')).toEqual(['ready'])
    expect(leaders.transition('first', 'workspace', 'stopping')).toEqual(['stopping'])
    expect(leaders.transition('first', 'workspace', 'stopped')).toEqual(['stopped'])

    const direct = runtime('direct', 'other')
    expect(leaders.add(direct)).toBe(true)
    expect(leaders.transition('direct', 'other', 'crashed')).toEqual(['crashed'])
    expect(leaders.transition('direct', 'other', 'stopped')).toEqual([])
  })
})
