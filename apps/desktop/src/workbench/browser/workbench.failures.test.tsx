import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import {
  createTestAPI,
  sessionDetail,
  sessionTwo,
} from '../../../test/support/workbench/api-stub'
import { Workbench } from './workbench'

describe('Workbench failures and sending', () => {
  it('surfaces Federation failures from the Conversation workspace', async () => {
    const { api } = createTestAPI({
      getMetaHubSettings: async () => { throw new Error('federation unavailable') },
    })
    render(<Workbench api={api} />)
    expect(await screen.findByRole('alert')).toHaveTextContent('federation unavailable')
  })

  it('shows bootstrap, open-session, and send failures without losing input', async () => {
    const bootstrapFailure = createTestAPI({
      bootstrap: async () => {
        throw new Error('bootstrap failed')
      },
    })
    const first = render(<Workbench api={bootstrapFailure.api} />)
    expect(await screen.findByRole('alert')).toHaveTextContent('bootstrap failed')
    first.unmount()

    const failedOpenSession = vi.fn(async () => {
      throw new Error('open failed')
    })
    const openFailure = createTestAPI({ openSession: failedOpenSession })
    const second = render(<Workbench api={openFailure.api} />)
    expect(await screen.findByRole('alert')).toHaveTextContent('open failed')
    fireEvent.click(screen.getByText('Version the protocol'))
    await waitFor(() => expect(failedOpenSession).toHaveBeenCalledTimes(2))
    second.unmount()

    const sendFailure = createTestAPI({
      sendMessage: async () => {
        throw 'send failed'
      },
    })
    render(<Workbench api={sendFailure.api} />)
    await screen.findByText('Conversation for Build the desktop workbench')
    const input = screen.getByLabelText('Message Loopal')
    fireEvent.change(input, { target: { value: 'Retry me' } })
    fireEvent.click(screen.getByRole('button', { name: 'Send' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('send failed')
    expect(input).toHaveValue('Retry me')
  })

  it('uses the first session fallback and guards empty or concurrent sends', async () => {
    let finishSend: (() => void) | undefined
    const sendMessage = vi.fn(
      () =>
        new Promise<void>((resolve) => {
          finishSend = resolve
        }),
    )
    const openSession = vi.fn(async () => sessionDetail(sessionTwo))
    const { api } = createTestAPI({
      bootstrap: async () => ({
        protocolVersion: 2,
        hostStatus: 'ready',
        workspaces: [],
        sessions: [sessionTwo],
        runtimes: [],
      }),
      openSession,
      sendMessage,
    })
    render(<Workbench api={api} />)
    const input = await screen.findByLabelText('Message Loopal')
    await waitFor(() => expect(openSession).toHaveBeenCalledWith(sessionTwo.id))

    fireEvent.keyDown(input, { key: 'Enter' })
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: true })
    fireEvent.keyDown(input, { key: 'x' })
    expect(sendMessage).not.toHaveBeenCalled()

    fireEvent.change(input, { target: { value: 'Run once' } })
    fireEvent.keyDown(input, { key: 'Enter' })
    await waitFor(() => expect(screen.getByRole('button', { name: 'Running…' })).toBeDisabled())
    fireEvent.change(input, { target: { value: 'Do not run twice' } })
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(sendMessage).toHaveBeenCalledTimes(1)

    finishSend?.()
    await waitFor(() => expect(screen.getByRole('button', { name: 'Send' })).toBeInTheDocument())
  })
})
