import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { richView } from '../../../../../test/fixtures/workbench/rich-session'
import {
  createStage2Callbacks,
  stage2Model,
} from '../../../../../test/fixtures/workbench/attention'
import {
  createTestAPI, sessionDetail, sessionOne,
} from '../../../../../test/support/workbench/api-stub'
import { Workbench } from '../../../browser/workbench'

describe('Workbench Agent control feedback', () => {
  it('keeps settings command failures visible in the central session surface', async () => {
    const detail = {
      ...sessionDetail(sessionOne),
      view: richView(),
    }
    const controlAgent = vi.fn(async () => { throw new Error('control denied') })
    const { api } = createTestAPI({ openSession: async () => detail, controlAgent })
    render(<Workbench api={api} stage2={{
      model: stage2Model,
      callbacks: createStage2Callbacks(),
    }} />)
    expect(await screen.findByTestId('active-session-title')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Settings' }))
    expect(await screen.findByTestId('settings-pane')).toBeInTheDocument()
    fireEvent.click(screen.getByTestId('settings-navigation')
      .querySelector('[data-section="agent"]')!)
    fireEvent.click(screen.getByRole('button', { name: 'Clear' }))
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('control denied'))
    expect(screen.getByTestId('settings-pane')).toBeInTheDocument()
  })
})
