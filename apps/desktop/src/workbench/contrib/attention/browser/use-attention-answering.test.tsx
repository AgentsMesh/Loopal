import { act, renderHook, waitFor } from '@testing-library/react'
import { createTestAPI, updatedAt } from '../../../../../test/support/workbench/api-stub'
import { type QuestionRequest } from '../../../../shared/contracts'
import { type Stage2WorkbenchModel } from '../../../browser/stage2-view-model'
import { useAttentionController } from './use-attention-controller'

const sessions: Stage2WorkbenchModel['context']['sessions'] = [{
  id: 'session', workspaceId: 'workspace', title: 'Session', state: 'running',
  runtimeId: 'runtime', runtimeGeneration: 2,
}]

const request: QuestionRequest = {
  id: 'question', sessionId: 'session', runtimeId: 'runtime', generation: 2,
  agentId: 'main', classifierRunning: false, createdAt: updatedAt,
  questions: [
    { question: 'Strategy?', allowMultiple: false, options: [
      { label: 'Direct', description: '' }, { label: 'Careful', description: '' },
    ] },
    { question: 'Checks?', allowMultiple: true, options: [
      { label: 'Fast', description: '' }, { label: 'Safe', description: '' },
    ] },
  ],
}

describe('AskUser answer parity', () => {
  it('combines single Other text, multiple choices, and custom text', async () => {
    const respondQuestion = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({ respondQuestion })
    const hook = renderHook(() => useAttentionController(
      api, sessions, 'session', true, vi.fn(),
    ))
    act(() => events.fire({ type: 'question_requested', request }))
    expect(hook.result.current.questions.at(-1)?.submit?.enabled).toBe(false)

    act(() => hook.result.current.callbacks.onAnswerQuestion?.(
      'main:question:0', '0:Direct',
    ))
    act(() => hook.result.current.callbacks.onQuestionFreeTextChange?.(
      'main:question:0', 'A custom path',
    ))
    act(() => hook.result.current.callbacks.onAnswerQuestion?.(
      'main:question:1', '0:Fast',
    ))
    act(() => hook.result.current.callbacks.onAnswerQuestion?.(
      'main:question:1', '1:Safe',
    ))
    act(() => hook.result.current.callbacks.onQuestionFreeTextChange?.(
      'main:question:1', 'with audit logs',
    ))

    expect(hook.result.current.questions[0]).toMatchObject({
      selectedChoiceIds: [], otherText: 'A custom path',
    })
    expect(hook.result.current.questions[1]).toMatchObject({
      selectedChoiceIds: ['0:Fast', '1:Safe'], otherText: 'with audit logs',
      submit: { enabled: true },
    })
    act(() => hook.result.current.callbacks.onSubmitQuestionAnswers?.('main:question'))
    await waitFor(() => expect(respondQuestion).toHaveBeenCalledWith({
      sessionId: 'session', runtimeId: 'runtime', generation: 2,
      agentId: 'main', requestId: 'question',
      answers: ['A custom path', 'Fast, Safe, with audit logs'],
    }))
  })

  it('cancels the whole pending request through the same runtime scope', async () => {
    const respondQuestion = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({ respondQuestion })
    const hook = renderHook(() => useAttentionController(
      api, sessions, 'session', true, vi.fn(),
    ))
    act(() => events.fire({ type: 'question_requested', request }))
    act(() => hook.result.current.callbacks.onCancelQuestion?.('main:question'))
    await waitFor(() => expect(respondQuestion).toHaveBeenCalledWith({
      sessionId: 'session', runtimeId: 'runtime', generation: 2,
      agentId: 'main', requestId: 'question', cancelled: true,
    }))
  })
})
