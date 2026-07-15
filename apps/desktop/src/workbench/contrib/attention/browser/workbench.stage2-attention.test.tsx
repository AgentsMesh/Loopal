import { fireEvent, render, screen, within } from '@testing-library/react'
import {
  createStage2Callbacks,
  stage2Model,
} from '../../../../../test/fixtures/workbench/attention'
import { createTestAPI } from '../../../../../test/support/workbench/api-stub'
import { Workbench } from '../../../browser/workbench'

describe('Stage 2 agent attention panes', () => {
  it('resolves permission requests and answers structured questions', async () => {
    const callbacks = createStage2Callbacks()
    const { api } = createTestAPI()
    render(<Workbench api={api} stage2={{ model: stage2Model, callbacks }} />)
    await screen.findByText('Conversation for Build the desktop workbench')

    expect(screen.getByTestId('session-attention')).toBeInTheDocument()
    fireEvent.click(screen.getByLabelText('3 pending requests'))
    const permissions = screen.getByTestId('permissions-pane')
    expect(permissions).toHaveTextContent('apply_patch')
    expect(permissions).toHaveTextContent('Agent main')
    expect(permissions).toHaveTextContent('Use network')
    const writeCard = screen.getByText('Write files').closest('article')!
    const networkCard = screen.getByText('Use network').closest('article')!
    fireEvent.click(within(writeCard).getByRole('button', { name: 'Allow' }))
    fireEvent.click(within(networkCard).getByRole('button', { name: 'Deny' }))
    fireEvent.click(within(writeCard).getByRole('button', { name: 'Allow for session' }))
    expect(callbacks.onResolvePermission).toHaveBeenNthCalledWith(1, 'write', 'allow')
    expect(callbacks.onResolvePermission).toHaveBeenNthCalledWith(2, 'network', 'deny')
    expect(callbacks.onResolvePermission).toHaveBeenNthCalledWith(3, 'write', 'allow_session')

    const questions = screen.getByTestId('questions-pane')
    expect(questions).toHaveTextContent('Use less space.')
    expect(questions).toHaveTextContent('Agent question · worker')
    fireEvent.click(within(questions).getByRole('button', { name: 'Comfortable' }))
    expect(callbacks.onAnswerQuestion).toHaveBeenCalledWith('style', 'comfortable')
  })

  it('renders empty attention states and tolerates absent adapters', async () => {
    const { api } = createTestAPI()
    const empty = { ...stage2Model, permissions: [], questions: [] }
    const first = render(<Workbench api={api} stage2={{ model: empty, callbacks: {} }} />)
    await screen.findByText('Conversation for Build the desktop workbench')
    expect(screen.queryByLabelText(/pending requests/)).not.toBeInTheDocument()
    expect(screen.queryByTestId('session-attention')).not.toBeInTheDocument()
    first.unmount()

    render(<Workbench api={api} stage2={{ model: stage2Model, callbacks: {} }} />)
    await screen.findByText('Conversation for Build the desktop workbench')
    fireEvent.click(screen.getAllByRole('button', { name: 'Allow' })[0]!)
    fireEvent.click(screen.getByText('Compact').closest('button')!)
  })
})
