import { act, renderHook, waitFor } from '@testing-library/react'
import { createTestAPI, updatedAt } from '../../../../../test/support/workbench/api-stub'
import { type Stage2WorkbenchModel } from '../../../browser/stage2-view-model'
import { useWorkbenchRuntimeController } from '../../../browser/use-workbench-runtime-controller'

function context(generation: number): Stage2WorkbenchModel['context'] {
  return {
    workspaces: [{ id: 'workspace', name: 'Loopal', detail: '/loopal' }],
    activeWorkspaceId: 'workspace',
    sessions: [{
      id: 'session', workspaceId: 'workspace', title: 'Session', state: 'running',
      runtimeId: `runtime-${generation}`, runtimeGeneration: generation,
    }],
    activeSessionId: 'session',
  }
}

describe('attention runtime generation isolation', () => {
  it('clears old pending requests and rejects late old-generation events', async () => {
    const respondPermission = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({ respondPermission })
    const hook = renderHook(
      ({ generation }) => useWorkbenchRuntimeController(api, context(generation), true),
      { initialProps: { generation: 1 } },
    )
    act(() => events.fire({
      type: 'permission_requested',
      request: {
        id: 'same-id', sessionId: 'session', runtimeId: 'runtime-1', generation: 1,
        agentId: 'main', tool: 'shell', title: 'Run', detail: 'test', risk: 'medium',
        createdAt: updatedAt,
      },
    }))
    expect(hook.result.current.model.permissions).toHaveLength(1)
    act(() => events.fire({
      type: 'permission_requested',
      request: {
        id: 'other-id', sessionId: 'session', runtimeId: 'runtime-1', generation: 1,
        agentId: 'main', tool: 'read', title: 'Other', detail: 'read', risk: 'low',
        createdAt: updatedAt,
      },
    }))
    act(() => events.fire({
      type: 'permission_requested',
      request: {
        id: 'same-id', sessionId: 'session', runtimeId: 'runtime-1', generation: 1,
        agentId: 'main', tool: 'shell', title: 'Updated', detail: 'new', risk: 'high',
        createdAt: updatedAt,
      },
    }))
    expect(hook.result.current.model.permissions).toHaveLength(2)
    expect(hook.result.current.model.permissions.find((item) => item.id === 'main:same-id')?.title)
      .toBe('Updated')
    act(() => hook.result.current.callbacks.onResolvePermission?.(
      'main:same-id', 'allow_session',
    ))
    await waitFor(() => expect(respondPermission).toHaveBeenCalledWith({
      sessionId: 'session', runtimeId: 'runtime-1', generation: 1,
      agentId: 'main', requestId: 'same-id', decision: 'allow_session',
    }))
    hook.rerender({ generation: 2 })
    await waitFor(() => expect(hook.result.current.model.permissions).toHaveLength(0))
    act(() => events.fire({
      type: 'permission_requested',
      request: {
        id: 'same-id', sessionId: 'session', runtimeId: 'runtime-1', generation: 1,
        agentId: 'main', tool: 'shell', title: 'Late', detail: 'old', risk: 'medium',
        createdAt: updatedAt,
      },
    }))
    expect(hook.result.current.model.permissions).toHaveLength(0)
  })

  it('returns the request runtime scope with an answer', async () => {
    const respondQuestion = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({ respondQuestion })
    const hook = renderHook(() => useWorkbenchRuntimeController(api, context(2), true))
    act(() => events.fire({
      type: 'question_requested',
      request: {
        id: 'question', sessionId: 'session', runtimeId: 'runtime-2', generation: 2,
        agentId: 'main', classifierRunning: false, createdAt: updatedAt,
        questions: [{
          question: 'Continue?', allowMultiple: false,
          options: [{ label: 'Yes', description: '' }],
        }],
      },
    }))
    act(() => hook.result.current.callbacks.onAnswerQuestion?.('main:question:0', '0:Yes'))
    await act(async () => {
      hook.result.current.callbacks.onSubmitQuestionAnswers?.('main:question')
      await Promise.resolve()
    })
    await waitFor(() => expect(respondQuestion).toHaveBeenCalledWith({
      sessionId: 'session', runtimeId: 'runtime-2', generation: 2,
      agentId: 'main', requestId: 'question', answers: ['Yes'],
    }))
  })

  it('clears partial answers when a scoped question resolves', async () => {
    const respondQuestion = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({ respondQuestion })
    const hook = renderHook(() => useWorkbenchRuntimeController(api, context(2), true))
    const request = {
      id: 'question', sessionId: 'session', runtimeId: 'runtime-2', generation: 2,
      agentId: 'main', classifierRunning: false, createdAt: updatedAt,
      questions: [
        { question: 'First?', allowMultiple: false,
          options: [{ label: 'A', description: '' }] },
        { question: 'Second?', allowMultiple: false,
          options: [{ label: 'B', description: '' }] },
      ],
    }
    act(() => events.fire({ type: 'question_requested', request }))
    act(() => hook.result.current.callbacks.onAnswerQuestion?.('main:question:9', '0:invalid'))
    await act(async () => {
      hook.result.current.callbacks.onAnswerQuestion?.('main:question:0', '0:A')
      await Promise.resolve()
    })
    await act(async () => {
      hook.result.current.callbacks.onSubmitQuestionAnswers?.('main:question')
      await Promise.resolve()
    })
    expect(respondQuestion).not.toHaveBeenCalled()
    act(() => events.fire({
      type: 'question_resolved', sessionId: 'session', runtimeId: 'runtime-2',
      generation: 2, agentId: 'main', requestId: 'question',
    }))
    act(() => events.fire({ type: 'question_requested', request }))
    await act(async () => {
      hook.result.current.callbacks.onAnswerQuestion?.('main:question:1', '0:B')
      await Promise.resolve()
    })
    expect(respondQuestion).not.toHaveBeenCalled()
  })

  it('routes denial failures and drops partial answers on generation change', async () => {
    const respondPermission = vi.fn(async () => { throw new Error('route failed') })
    const { api, events } = createTestAPI({ respondPermission })
    const hook = renderHook(
      ({ generation }) => useWorkbenchRuntimeController(api, context(generation), true),
      { initialProps: { generation: 2 } },
    )
    act(() => events.fire({
      type: 'permission_requested',
      request: {
        id: 'permission', sessionId: 'session', runtimeId: 'runtime-2', generation: 2,
        agentId: 'main', tool: 'shell', title: 'Run', detail: 'test', risk: 'high',
        createdAt: updatedAt,
      },
    }))
    act(() => hook.result.current.callbacks.onResolvePermission?.('main:permission', 'deny'))
    await waitFor(() => expect(respondPermission).toHaveBeenCalledWith({
      sessionId: 'session', runtimeId: 'runtime-2', generation: 2,
      agentId: 'main', requestId: 'permission', decision: 'deny',
    }))
    await waitFor(() => expect(hook.result.current.model.error).toBe('route failed'))

    act(() => events.fire({
      type: 'question_requested',
      request: {
        id: 'partial', sessionId: 'session', runtimeId: 'runtime-2', generation: 2,
        agentId: 'main', classifierRunning: false, createdAt: updatedAt,
        questions: [{
          question: 'Pick?', allowMultiple: true,
          options: [{ label: 'A', description: '' }],
        }],
      },
    }))
    act(() => hook.result.current.callbacks.onAnswerQuestion?.('main:partial:0', '0:A'))
    hook.rerender({ generation: 3 })
    await waitFor(() => expect(hook.result.current.model.questions).toHaveLength(0))
  })
})
