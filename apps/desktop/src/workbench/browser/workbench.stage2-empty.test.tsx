import { render, screen } from '@testing-library/react'
import { stage2Model } from '../../../test/fixtures/workbench/attention'
import { createTestAPI } from '../../../test/support/workbench/api-stub'
import { Workbench } from './workbench'

describe('Stage 2 empty surfaces', () => {
  it('keeps conversation usable without runtime resources', async () => {
    const { api } = createTestAPI()
    const model = {
      ...stage2Model,
      context: { workspaces: [], sessions: [] },
    }
    render(<Workbench api={api} stage2={{ model, callbacks: {} }} />)
    await screen.findByText('Conversation for Build the desktop workbench')

    expect(screen.queryByLabelText('Active workspace')).not.toBeInTheDocument()
    expect(screen.queryByLabelText('Active session')).not.toBeInTheDocument()
    expect(screen.getByTestId('active-session-title')).toHaveTextContent('Build')
    expect(screen.queryByRole('button', { name: 'Terminal' })).not.toBeInTheDocument()
    expect(screen.queryByTestId('terminal-panel')).not.toBeInTheDocument()
  })
})
