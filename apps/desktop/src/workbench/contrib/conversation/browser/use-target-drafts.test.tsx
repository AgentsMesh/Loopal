import { act, renderHook } from '@testing-library/react'
import { useTargetDrafts } from './use-target-drafts'

describe('useTargetDrafts', () => {
  it('keeps drafts isolated by session and Agent target', () => {
    const hook = renderHook(
      ({ sessionId }: { sessionId: string }) => useTargetDrafts(sessionId),
      { initialProps: { sessionId: 'session-a' } },
    )
    act(() => {
      hook.result.current.set('main', 'root draft')
      hook.result.current.set('child', 'child draft')
    })
    expect(hook.result.current.get('main')).toBe('root draft')
    expect(hook.result.current.get('child')).toBe('child draft')

    hook.rerender({ sessionId: 'session-b' })
    expect(hook.result.current.get('main')).toBe('')
    act(() => hook.result.current.set('main', 'other session'))
    hook.rerender({ sessionId: 'session-a' })
    expect(hook.result.current.get('main')).toBe('root draft')
    expect(hook.result.current.get('child')).toBe('child draft')
  })
})
