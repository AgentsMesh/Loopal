import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { createTestAPI, updatedAt } from '../../../../../test/support/workbench/api-stub'
import { Workbench } from '../../../browser/workbench'

const intentDigest = `sha256:${'ab'.repeat(32)}`

describe('Workbench attention runtime surfaces', () => {
  it('projects permission and multi-question events into actionable panes', async () => {
    const respondPermission = vi.fn(async () => undefined)
    const respondQuestion = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({ respondPermission, respondQuestion })
    render(<Workbench api={api} />)
    await screen.findByText('Conversation for Build the desktop workbench')
    act(() => {
      events.fire({
        type: 'permission_requested',
        request: {
          id: 'permission', sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
          agentId: 'main', tool: 'shell', intentDigest,
          title: 'Run tests', detail: 'bazel test //...', risk: 'medium', createdAt: updatedAt,
        },
      })
      events.fire({
        type: 'question_requested',
        request: {
          id: 'question', sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
          agentId: 'main', classifierRunning: true,
          classifierStatus: { kind: 'running', elapsedMs: 1_500 },
          createdAt: updatedAt, questions: [
            { question: 'Mode?', header: 'Execution', allowMultiple: false,
              options: [{ label: 'Fast', description: 'Run focused tests' }] },
            { question: 'Continue?', allowMultiple: false,
              options: [{ label: 'Yes', description: '' }] },
          ],
        },
      })
    })
    expect(screen.getByLabelText('3 pending requests')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Allow' }))
    await waitFor(() => expect(respondPermission).toHaveBeenCalledWith({
      sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
      agentId: 'main', requestId: 'permission', intentDigest, decision: 'allow_once',
    }))
    act(() => events.fire({
      type: 'permission_resolved', sessionId: 'session-1', runtimeId: 'runtime-1',
      generation: 1, agentId: 'main', requestId: 'permission',
    }))
    expect(screen.queryByTestId('permissions-pane')).not.toBeInTheDocument()

    const questions = screen.getByTestId('questions-pane')
    expect(questions).toHaveTextContent('Auto-answering · 1.5s')
    fireEvent.click(within(questions).getByText('Fast').closest('button')!)
    expect(respondQuestion).not.toHaveBeenCalled()
    fireEvent.click(within(questions).getByText('Yes').closest('button')!)
    expect(respondQuestion).not.toHaveBeenCalled()
    fireEvent.click(within(questions).getByRole('button', { name: 'Submit answers' }))
    await waitFor(() => expect(respondQuestion).toHaveBeenCalledWith({
      sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
      agentId: 'main', requestId: 'question', answers: ['Fast', 'Yes'],
    }))
    act(() => events.fire({
      type: 'question_resolved', sessionId: 'session-1', runtimeId: 'runtime-1',
      generation: 1, agentId: 'main', requestId: 'question',
    }))
    expect(screen.queryByTestId('questions-pane')).not.toBeInTheDocument()
  })

  it('toggles multiple choices and submits them explicitly', async () => {
    const respondQuestion = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({ respondQuestion })
    render(<Workbench api={api} />)
    await screen.findByText('Conversation for Build the desktop workbench')
    act(() => events.fire({
      type: 'question_requested',
      request: {
        id: 'multi', sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
        agentId: 'main', classifierRunning: false, createdAt: updatedAt,
        questions: [
          {
            question: 'Select tools', allowMultiple: true,
            options: [
              { label: 'A', description: '' },
              { label: 'B', description: '' },
              { label: 'C', description: '' },
            ],
          },
          {
            question: 'Continue?', allowMultiple: false,
            options: [{ label: 'Yes', description: '' }],
          },
        ],
      },
    }))
    const pane = screen.getByTestId('questions-pane')
    const choiceA = within(pane).getByRole('button', { name: 'A' })
    const choiceC = within(pane).getByRole('button', { name: 'C' })
    expect(within(pane).getByRole('button', { name: 'Submit answers' })).toBeDisabled()
    fireEvent.click(choiceA)
    expect(choiceA).toHaveAttribute('aria-pressed', 'true')
    fireEvent.click(choiceA)
    expect(choiceA).toHaveAttribute('aria-pressed', 'false')
    fireEvent.click(choiceA)
    fireEvent.click(choiceC)
    expect(choiceA).toHaveAttribute('aria-pressed', 'true')
    expect(choiceC).toHaveAttribute('aria-pressed', 'true')
    expect(respondQuestion).not.toHaveBeenCalled()
    const submit = within(pane).getByRole('button', { name: 'Submit answers' })
    expect(submit).toBeDisabled()
    fireEvent.click(within(pane).getByRole('button', { name: 'Yes' }))
    expect(submit).toBeEnabled()
    expect(respondQuestion).not.toHaveBeenCalled()
    fireEvent.click(submit)
    await waitFor(() => expect(respondQuestion).toHaveBeenCalledWith({
      sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
      agentId: 'main', requestId: 'multi', answers: ['A, C', 'Yes'],
    }))
  })
})
