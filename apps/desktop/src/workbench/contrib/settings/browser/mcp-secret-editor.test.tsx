import { fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { McpSecretEditor } from './mcp-secret-editor'
import { newMcpServerDraft, type McpServerDraft } from './mcp-server-draft'

function Harness(props: { initial: McpServerDraft; disabled?: boolean }): React.JSX.Element {
  const [draft, setDraft] = useState(props.initial)
  return <><McpSecretEditor draft={draft} disabled={props.disabled ?? false}
    onChange={setDraft} /><output data-testid="draft">{JSON.stringify(draft)}</output></>
}

describe('McpSecretEditor', () => {
  it('replaces, clears, removes, and restores configured environment secrets', () => {
    render(<Harness initial={{
      ...newMcpServerDraft(), secrets: [
        { name: 'TOKEN', configured: true }, { name: 'EMPTY', configured: false },
      ],
    }} />)
    expect(screen.getByText('Environment variables · values are write-only')).toBeVisible()
    expect(screen.getByText('configured · preserved')).toBeVisible()
    expect(screen.getByText('not configured')).toBeVisible()
    const value = screen.getByLabelText('Secret value TOKEN')
    fireEvent.change(value, { target: { value: 'replacement' } })
    expect(value).toHaveValue('replacement')
    expect(screen.getByText('set')).toBeVisible()
    expect(screen.getByTestId('draft')).toHaveTextContent('replacement')
    fireEvent.change(value, { target: { value: '' } })
    expect(screen.getByText('configured · preserved')).toBeVisible()
    const remove = screen.getByRole('button', { name: 'Remove secret TOKEN' })
    fireEvent.click(remove)
    expect(screen.getByText('remove')).toBeVisible()
    expect(remove).toHaveTextContent('Undo')
    fireEvent.click(remove)
    expect(screen.getByText('configured · preserved')).toBeVisible()
  })

  it('sets, edits, clears, and cancels a new write-only secret', () => {
    render(<Harness initial={newMcpServerDraft()} />)
    const name = screen.getByLabelText('New env name')
    const value = screen.getByLabelText('New env value')
    const set = screen.getByRole('button', { name: 'Set secret' })
    expect(set).toBeDisabled()
    fireEvent.change(name, { target: { value: 'TOKEN' } })
    expect(set).toBeDisabled()
    fireEvent.change(value, { target: { value: 'first' } })
    fireEvent.click(set)
    expect(name).toHaveValue('')
    expect(value).toHaveValue('')
    expect(screen.getByText('will be configured')).toBeVisible()
    const pending = screen.getByLabelText('Secret value TOKEN')
    fireEvent.change(pending, { target: { value: 'second' } })
    expect(screen.getByTestId('draft')).toHaveTextContent('second')
    fireEvent.change(pending, { target: { value: '' } })
    expect(screen.queryByText('will be configured')).not.toBeInTheDocument()

    fireEvent.change(name, { target: { value: 'TOKEN' } })
    fireEvent.change(value, { target: { value: 'third' } })
    fireEvent.click(set)
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(screen.queryByText('will be configured')).not.toBeInTheDocument()
  })

  it('emits header-target patches and disables every field while busy', () => {
    const initial: McpServerDraft = {
      ...newMcpServerDraft(), type: 'streamable-http', url: 'https://example.test/mcp',
      secrets: [{ name: 'Authorization', configured: true }],
    }
    const view = render(<Harness initial={initial} />)
    expect(screen.getByText('HTTP headers · values are write-only')).toBeVisible()
    fireEvent.change(screen.getByLabelText('Secret value Authorization'), {
      target: { value: 'Bearer replacement' },
    })
    expect(screen.getByTestId('draft')).toHaveTextContent('"target":"header"')
    view.unmount()
    render(<Harness initial={initial} disabled />)
    expect(screen.getByRole('group')).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Set secret' })).toBeDisabled()
  })
})
