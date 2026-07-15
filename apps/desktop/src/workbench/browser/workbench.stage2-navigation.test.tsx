import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import {
  createStage2Callbacks,
  stage2Model,
} from '../../../test/fixtures/workbench/attention'
import { createTestAPI } from '../../../test/support/workbench/api-stub'
import { Workbench } from './workbench'

describe('Stage 2 workbench navigation', () => {
  it('navigates context, settings, and conversation', async () => {
    const callbacks = createStage2Callbacks()
    const { api } = createTestAPI()
    render(<Workbench api={api} stage2={{ model: stage2Model, callbacks }} />)
    await screen.findByText('Conversation for Build the desktop workbench')

    expect(screen.getByLabelText('3 pending requests')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }))
    fireEvent.click(screen.getByTestId('settings-navigation')
      .querySelector('[data-section="runtime"]')!)
    expect(screen.getByTestId('diagnostics-pane')).toBeInTheDocument()
    expect(screen.queryByLabelText('Active workspace')).not.toBeInTheDocument()
    expect(screen.queryByLabelText('Active session')).not.toBeInTheDocument()
    expect(callbacks.onWorkspaceChange).not.toHaveBeenCalled()
    expect(callbacks.onSessionChange).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: 'Close settings' }))
    fireEvent.click(screen.getByRole('button', { name: 'Conversation' }))
    expect(screen.getByTestId('session-list')).toBeInTheDocument()
    await waitFor(() => expect(screen.getByTestId('active-session-title')).toHaveTextContent('Build'))
  })
})
