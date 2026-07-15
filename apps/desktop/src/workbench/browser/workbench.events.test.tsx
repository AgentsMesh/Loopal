import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import {
  createTestAPI,
  sessionOne,
  sessionTwo,
  updatedAt,
} from '../../../test/support/workbench/api-stub'
import { Workbench } from './workbench'

describe('Workbench events', () => {
  it('renders live host, session, conversation, and artifact events', async () => {
    const sendMessage = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({ sendMessage })
    render(<Workbench api={api} />)
    await screen.findByText('Conversation for Build the desktop workbench')

    await act(async () => {
      events.fire({ type: 'host_status', status: 'alive' })
      events.fire({
        type: 'session_updated',
        session: { ...sessionOne, status: 'running', attention: 'completed' },
      })
      events.fire({
        type: 'conversation_entry',
        sessionId: sessionOne.id,
        entry: {
          id: 'message-live',
          role: 'assistant',
          text: 'Live response',
          createdAt: updatedAt,
        },
      })
      events.fire({
        type: 'artifact_created',
        artifact: {
          id: 'artifact-live',
          sessionId: sessionOne.id,
          title: 'Live artifact.md',
          kind: 'report',
          uri: 'loopal-artifact://live',
          mediaType: 'text/markdown',
          producerAgentId: 'agent-session-1',
          createdAt: updatedAt,
        },
      })
      events.fire({ type: 'session_updated', session: { ...sessionTwo, status: 'failed' } })
    })

    expect(screen.queryByTestId('host-status')).not.toBeInTheDocument()
    expect(screen.getByTestId('session-list')
      .querySelector(`[data-session-id="${sessionOne.id}"] .attention`)).toBeNull()
    expect(screen.getByText('Live response')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('tab', { name: /Artifacts/ }))
    const artifact = screen.getByRole('button', { name: /Live artifact\.md/ })
    fireEvent.click(artifact)
    expect(artifact).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByText('loopal-artifact://live')).toBeInTheDocument()
    fireEvent.click(artifact)
    expect(screen.queryByText('loopal-artifact://live')).not.toBeInTheDocument()

    act(() => {
      events.fire({
        type: 'session_detail_replaced',
        detail: {
          session: { ...sessionOne, status: 'waiting', attention: 'question' },
          conversation: [{
            id: 'snapshot-message',
            role: 'assistant',
            text: 'Resynchronized response',
            createdAt: updatedAt,
          }],
          agents: [],
          artifacts: [],
        },
      })
    })
    expect(screen.getByText('Resynchronized response')).toBeInTheDocument()
    expect(screen.queryByText('Live response')).not.toBeInTheDocument()

    const input = screen.getByLabelText('Message Loopal')
    fireEvent.change(input, { target: { value: 'Ship it' } })
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: false })
    await waitFor(() => expect(sendMessage).toHaveBeenCalledWith(
      sessionOne.id, 'Ship it', 'main',
    ))
    expect(input).toHaveValue('')
  })

  it('ignores events belonging to another active session', async () => {
    const { api, events } = createTestAPI()
    render(<Workbench api={api} />)
    await screen.findByText('Conversation for Build the desktop workbench')

    act(() => {
      events.fire({
        type: 'conversation_entry',
        sessionId: sessionTwo.id,
        entry: {
          id: 'other',
          role: 'assistant',
          text: 'Other session event',
          createdAt: updatedAt,
        },
      })
      events.fire({
        type: 'artifact_created',
        artifact: {
          id: 'other-artifact',
          sessionId: sessionTwo.id,
          title: 'Other artifact',
          kind: 'other',
          uri: 'loopal-artifact://other',
          mediaType: 'text/plain',
          producerAgentId: 'agent-other',
          createdAt: updatedAt,
        },
      })
      events.fire({
        type: 'session_detail_replaced',
        detail: { session: sessionTwo, conversation: [], agents: [], artifacts: [] },
      })
    })

    expect(screen.queryByText('Other session event')).not.toBeInTheDocument()
    expect(screen.queryByText('Other artifact')).not.toBeInTheDocument()
  })
})
