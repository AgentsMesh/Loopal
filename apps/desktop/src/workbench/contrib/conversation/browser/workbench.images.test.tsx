import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { createTestAPI, sessionOne } from '../../../../../test/support/workbench/api-stub'
import { Workbench } from '../../../browser/workbench'

const image = {
  name: 'diagram.png', mediaType: 'image/png' as const,
  data: 'iVBORw==', sizeBytes: 4,
}

describe('Workbench image attachments', () => {
  it('selects, removes, and sends an image-only message', async () => {
    const selectImages = vi.fn(async () => [image])
    const sendMessage = vi.fn(async () => undefined)
    const { api } = createTestAPI({ selectImages, sendMessage })
    render(<Workbench api={api} />)
    await screen.findByText(`Conversation for ${sessionOne.title}`)

    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Attach images' }))
    expect(await screen.findByTestId('pending-image-attachments')).toHaveTextContent('diagram.png')
    expect(screen.getByRole('button', { name: 'Send' })).toBeEnabled()

    fireEvent.click(screen.getByRole('button', { name: 'Remove diagram.png' }))
    expect(screen.queryByTestId('pending-image-attachments')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Attach images' }))
    await screen.findByTestId('pending-image-attachments')
    fireEvent.click(screen.getByRole('button', { name: 'Send' }))

    await waitFor(() => expect(sendMessage).toHaveBeenCalledWith(
      sessionOne.id, '', 'agent-session-1', [image],
    ))
    expect(screen.queryByTestId('pending-image-attachments')).not.toBeInTheDocument()
  })

  it('restores images when message routing fails', async () => {
    const { api } = createTestAPI({
      selectImages: async () => [image],
      sendMessage: async () => { throw new Error('route failed') },
    })
    render(<Workbench api={api} />)
    await screen.findByText(`Conversation for ${sessionOne.title}`)
    fireEvent.click(screen.getByRole('button', { name: 'Attach images' }))
    await screen.findByTestId('pending-image-attachments')
    fireEvent.click(screen.getByRole('button', { name: 'Send' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('route failed')
    expect(screen.getByTestId('pending-image-attachments')).toHaveTextContent('diagram.png')
  })
})
