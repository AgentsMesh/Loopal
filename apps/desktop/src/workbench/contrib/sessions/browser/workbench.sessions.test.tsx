import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import {
  createTestAPI, sessionDetail, sessionOne, updatedAt,
} from '../../../../../test/support/workbench/api-stub'
import { Workbench } from '../../../browser/workbench'

const workspace = {
  id: 'workspace', name: 'Loopal', rootUri: '/loopal', kind: 'folder' as const,
}

describe('Workbench session lifecycle', () => {
  it('creates, stops, and restarts the selected session', async () => {
    const created = {
      ...sessionOne, id: 'session-new', title: 'New session',
      activeRuntimeId: 'runtime-new', createdAt: updatedAt, updatedAt,
    }
    const createSession = vi.fn(async () => sessionDetail(created))
    const authorizationId = 'd10f67f2-f471-44ea-b6d1-e1b963e11228'
    const selectSessionDirectory = vi.fn(async () => ({
      authorizationId, path: '/loopal', name: 'Loopal', suggestedWorktreeName: 'loopal-task',
      git: { root: '/loopal', branch: 'main', dirty: false },
    }))
    const stopSession = vi.fn(async () => undefined)
    const restartSession = vi.fn(async () => ({
      id: 'runtime-new-2', sessionId: created.id, workspaceId: workspace.id,
      generation: 2, state: 'ready' as const, rootAgent: 'main', startedAt: updatedAt,
    }))
    const { api } = createTestAPI({
      bootstrap: async () => ({
        protocolVersion: 2, hostStatus: 'ready', workspaces: [workspace],
        sessions: [sessionOne], runtimes: [], activeSessionId: sessionOne.id,
      }),
      createSession, selectSessionDirectory, stopSession, restartSession,
    })
    render(<Workbench api={api} />)
    await screen.findByText(`Conversation for ${sessionOne.title}`)
    expect(screen.getByTestId('message-composer')).toContainElement(
      screen.getByTestId('runtime-status'),
    )
    expect(screen.queryByTestId('host-status')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Session details' }))
    expect(screen.getByTestId('session-metadata')).toHaveTextContent(sessionOne.id)
    expect(screen.getByTestId('session-metadata')).toHaveTextContent(sessionOne.model)
    fireEvent.click(screen.getByRole('button', { name: 'Session details' }))
    expect(screen.queryByTestId('session-metadata')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'New Session' }))
    expect(screen.getByTestId('new-session-dialog')).toBeInTheDocument()
    expect(createSession).not.toHaveBeenCalled()
    fireEvent.click(screen.getByTestId('session-directory'))
    await waitFor(() => expect(selectSessionDirectory).toHaveBeenCalledOnce())
    fireEvent.click(screen.getByTestId('create-session-confirm'))
    await waitFor(() => expect(createSession).toHaveBeenCalledWith({
      authorizationId, launchMode: 'directory',
    }))
    expect(screen.queryByTestId('new-session-dialog')).not.toBeInTheDocument()
    expect(screen.getByTestId('active-session-title')).toHaveTextContent('New session')
    const composer = within(screen.getByTestId('message-composer'))
    fireEvent.click(composer.getByRole('button', { name: 'Stop session' }))
    await waitFor(() => expect(stopSession).toHaveBeenCalledWith(created.id))
    fireEvent.click(composer.getByRole('button', { name: 'Restart session' }))
    await waitFor(() => expect(restartSession).toHaveBeenCalledWith(created.id))
  })

  it('disables message input for a stopped session', async () => {
    const stopped = {
      ...sessionOne, status: 'stopped' as const, activeRuntimeId: undefined,
    }
    const { api, events } = createTestAPI({
      openSession: async () => sessionDetail(stopped),
    })
    render(<Workbench api={api} />)
    await waitFor(() => expect(screen.getByLabelText('Message Loopal')).toBeDisabled())
    await act(async () => {
      events.fire({ type: 'session_updated', session: stopped })
      await Promise.resolve()
    })
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled()
  })

  it('gates failed and archived sessions by actual runtime lifecycle', async () => {
    const { activeRuntimeId: _runtime, ...inactive } = sessionOne
    const failed = {
      ...inactive, status: 'failed' as const,
      attention: 'failure' as const,
    }
    const { api, events } = createTestAPI({
      openSession: async () => sessionDetail(failed),
    })
    render(<Workbench api={api} />)
    await waitFor(() => expect(
      screen.getByRole('button', { name: 'Restart session' }),
    ).toBeEnabled())
    expect(screen.getByLabelText('Message Loopal')).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Stop session' })).toBeDisabled()

    const { attention: _attention, ...withoutAttention } = failed
    const archived = { ...withoutAttention, status: 'archived' as const }
    await act(async () => {
      events.fire({ type: 'session_updated', session: archived })
      await Promise.resolve()
    })
    expect(screen.getByRole('button', { name: 'Stop session' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Restart session' })).toBeDisabled()
    expect(screen.getByLabelText('Message Loopal')).toBeDisabled()
  })

  it('uses the root target when a live session has no projected agents yet', async () => {
    const empty = {
      ...sessionDetail({ ...sessionOne, mode: 'act' }),
      agents: [], view: undefined,
    }
    const sendMessage = vi.fn(async () => undefined)
    const controlAgent = vi.fn(async () => undefined)
    const { api } = createTestAPI({
      openSession: async () => empty, sendMessage, controlAgent,
    })
    render(<Workbench api={api} />)
    await screen.findByText(`Conversation for ${sessionOne.title}`)
    fireEvent.change(screen.getByRole('combobox', { name: 'Agent mode' }), {
      target: { value: 'plan' },
    })
    expect(controlAgent).not.toHaveBeenCalled()
    const composer = screen.getByLabelText('Message Loopal')
    fireEvent.change(composer, { target: { value: 'Queue for root' } })
    fireEvent.click(screen.getByRole('button', { name: 'Send' }))
    await waitFor(() => expect(sendMessage).toHaveBeenCalledWith(
      sessionOne.id, 'Queue for root', 'main',
    ))
  })

  it('routes more actions and mode changes through the exact agent generation', async () => {
    const root = {
      id: 'agent-session-1', name: 'Loopal', status: 'running' as const, mode: 'act',
    }
    const detail = { ...sessionDetail(sessionOne), agents: [root] }
    const interruptAgent = vi.fn(async () => undefined)
    const controlAgent = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({
      openSession: async () => detail,
      interruptAgent,
      controlAgent,
    })
    render(<Workbench api={api} />)
    await screen.findByText(`Conversation for ${sessionOne.title}`)
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }))
    fireEvent.click(screen.getByTestId('settings-navigation')
      .querySelector('[data-section="agent"]')!)
    const actions = screen.getByRole('group', { name: 'Agent controls' })
    const target = {
      sessionId: sessionOne.id, runtimeId: 'runtime-1', generation: 1,
      agentId: root.id,
    }
    fireEvent.click(within(actions).getByRole('button', { name: 'Interrupt' }))
    await waitFor(() => expect(interruptAgent).toHaveBeenCalledWith(target))
    for (const [label, command] of [
      ['Compact', { type: 'compact' }],
      ['Clear', { type: 'clear' }],
      ['Suspend', { type: 'suspend' }],
    ] as const) {
      fireEvent.click(within(actions).getByRole('button', { name: label }))
      await waitFor(() => expect(controlAgent).toHaveBeenCalledWith({ target, command }))
    }
    fireEvent.change(within(actions).getByRole('combobox', { name: 'Agent mode' }), {
      target: { value: 'plan' },
    })
    await waitFor(() => expect(controlAgent).toHaveBeenCalledWith({
      target, command: { type: 'mode', mode: 'plan' },
    }))

    await act(async () => {
      events.fire({
        type: 'session_detail_replaced',
        detail: { ...detail, agents: [{ ...root, status: 'suspended' }] },
      })
      await Promise.resolve()
    })
    fireEvent.click(within(actions).getByRole('button', { name: 'Unsuspend' }))
    await waitFor(() => expect(controlAgent).toHaveBeenCalledWith({
      target, command: { type: 'unsuspend' },
    }))
  })

  it('shows an unsupported observed mode without inventing Act or Plan controls', async () => {
    const detail = sessionDetail(sessionOne)
    const { api } = createTestAPI({ openSession: async () => detail })
    render(<Workbench api={api} />)
    await screen.findByText(`Conversation for ${sessionOne.title}`)
    expect(screen.queryByRole('combobox', { name: 'Agent mode' })).not.toBeInTheDocument()
    expect(screen.getByText('agent · Loopal Agent')).toBeInTheDocument()
  })

})
