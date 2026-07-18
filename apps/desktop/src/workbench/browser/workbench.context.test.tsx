import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import {
  createTestAPI,
  sessionDetail,
  sessionOne,
  updatedAt,
} from '../../../test/support/workbench/api-stub'
import { Workbench } from './workbench'

const docsSession = {
  ...sessionOne,
  id: 'docs-session',
  workspaceId: 'docs',
  title: 'Write the guide',
  updatedAt,
}

const workspaces = [
  { id: 'workspace', name: 'Loopal', rootUri: '/work/loopal', kind: 'folder' as const },
  { id: 'docs', name: 'Docs', rootUri: '/work/docs', kind: 'git_worktree' as const },
]

describe('Workbench session catalog', () => {
  it('opens live sessions across workspace scopes from one catalog', async () => {
    const openSession = vi.fn(async (sessionId: string) => (
      sessionDetail(sessionId === docsSession.id ? docsSession : sessionOne)
    ))
    const { api } = createTestAPI({
      bootstrap: async () => ({
        protocolVersion: 2,
        hostStatus: 'ready',
        workspaces,
        sessions: [sessionOne, docsSession],
        runtimes: [],
        activeSessionId: sessionOne.id,
      }),
      openSession,
    })
    render(<Workbench api={api} />)
    await screen.findByText('Conversation for Build the desktop workbench')
    expect(screen.queryByLabelText('Active workspace')).not.toBeInTheDocument()
    expect(screen.queryByLabelText('Active session')).not.toBeInTheDocument()
    const list = within(screen.getByTestId('session-list'))
    expect(list.getByText('Build the desktop workbench')).toBeInTheDocument()
    expect(list.getByText('Write the guide')).toBeInTheDocument()

    fireEvent.click(list.getByText('Write the guide'))
    await waitFor(() => expect(openSession).toHaveBeenCalledWith(docsSession.id))
    // The header re-renders after the openSession promise resolves — waiting
    // on the mock call alone races that commit.
    await waitFor(() => expect(screen.getByTestId('active-session-title'))
      .toHaveTextContent('Write the guide'))
    expect(screen.getByText('Conversation for Write the guide')).toBeInTheDocument()
  })

  it('allows session creation without a workspace or active session', async () => {
    const { api } = createTestAPI({
      bootstrap: async () => ({
        protocolVersion: 2,
        hostStatus: 'ready',
        workspaces: [],
        sessions: [],
        runtimes: [],
      }),
    })
    render(<Workbench api={api} />)
    await screen.findByText('No active sessions. Create one to start working.')
    expect(screen.queryByLabelText('Active workspace')).not.toBeInTheDocument()
    expect(screen.queryByLabelText('Active session')).not.toBeInTheDocument()
    const create = screen.getByRole('button', { name: 'New Session' })
    expect(create).toBeEnabled()
    fireEvent.click(create)
    expect(screen.getByRole('dialog')).toBeInTheDocument()
  })
})
