import { fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { McpServerForm } from './mcp-server-form'
import { newMcpServerDraft, type McpServerDraft } from './mcp-server-draft'

function Harness(props: {
  initial: McpServerDraft
  busy?: boolean
  onSave?: () => void
  onCancel?: () => void
}): React.JSX.Element {
  const [draft, setDraft] = useState(props.initial)
  return <><McpServerForm draft={draft} busy={props.busy ?? false} onChange={setDraft}
    onSave={props.onSave ?? vi.fn()} onCancel={props.onCancel ?? vi.fn()} />
    <output data-testid="draft">{JSON.stringify(draft)}</output></>
}

describe('McpServerForm', () => {
  it('edits every stdio and cwd field, submits, and cancels', () => {
    const onSave = vi.fn()
    const onCancel = vi.fn()
    render(<Harness initial={newMcpServerDraft()} onSave={onSave} onCancel={onCancel} />)
    fireEvent.change(screen.getByLabelText('MCP server name'), { target: { value: 'tools' } })
    fireEvent.change(screen.getByLabelText('MCP sharing'), { target: { value: 'per-agent' } })
    fireEvent.change(screen.getByLabelText('MCP timeout milliseconds'), {
      target: { value: '12000' },
    })
    fireEvent.click(screen.getByLabelText('Enable MCP server'))
    fireEvent.change(screen.getByLabelText('MCP command'), { target: { value: 'node' } })
    fireEvent.change(screen.getByLabelText('MCP arguments'), {
      target: { value: ' server.js \n\n--stdio' },
    })
    expect(screen.queryByLabelText('Cwd isolation argument')).not.toBeInTheDocument()
    fireEvent.click(screen.getByLabelText('Use MCP cwd isolation'))
    fireEvent.change(screen.getByLabelText('Cwd isolation argument'), {
      target: { value: '--profile' },
    })
    fireEvent.change(screen.getByLabelText('Cwd isolation cache subdirectory'), {
      target: { value: 'tools' },
    })
    expect(screen.getByTestId('draft')).toHaveTextContent('"sharing":"per-agent"')
    expect(screen.getByTestId('draft')).toHaveTextContent('"enabled":false')
    expect(screen.getByTestId('draft')).toHaveTextContent(' server.js ')
    expect(screen.getByTestId('draft')).toHaveTextContent('--profile')
    fireEvent.submit(screen.getByRole('button', { name: 'Save MCP server' }).closest('form')!)
    expect(onSave).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(onCancel).toHaveBeenCalledOnce()
  })

  it('switches to HTTP, clears transport secrets, and edits the URL', () => {
    render(<Harness initial={{
      ...newMcpServerDraft(), name: 'tools', secretPatches: [{
        target: 'env', name: 'TOKEN', operation: 'remove',
      }], secrets: [{ name: 'TOKEN', configured: true }],
    }} />)
    fireEvent.change(screen.getByLabelText('MCP transport'), {
      target: { value: 'streamable-http' },
    })
    expect(screen.queryByLabelText('MCP command')).not.toBeInTheDocument()
    expect(screen.getByText('HTTP headers · values are write-only')).toBeVisible()
    fireEvent.change(screen.getByLabelText('MCP HTTP URL'), {
      target: { value: 'https://example.test/mcp' },
    })
    expect(screen.getByTestId('draft')).toHaveTextContent('"type":"streamable-http"')
    expect(screen.getByTestId('draft')).toHaveTextContent('https://example.test/mcp')
    expect(screen.getByTestId('draft')).toHaveTextContent('"secrets":[]')
    expect(screen.getByTestId('draft')).toHaveTextContent('"secretPatches":[]')
  })

  it('shows inherited-secret guidance and honors locked and busy states', () => {
    const draft = {
      ...newMcpServerDraft(), lockedName: true, restrictedSecrets: true,
      type: 'streamable-http' as const, name: 'remote', url: 'https://example.test/mcp',
    }
    const view = render(<Harness initial={draft} />)
    expect(screen.getByRole('note')).toHaveTextContent('cannot be copied')
    expect(screen.getByLabelText('MCP server name')).toBeDisabled()
    expect(screen.getByLabelText('MCP HTTP URL')).toBeEnabled()
    view.unmount()
    render(<Harness initial={{ ...draft, lockedName: false }} busy />)
    expect(screen.getByLabelText('MCP server name')).toBeDisabled()
    expect(screen.getByLabelText('MCP HTTP URL')).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Save MCP server' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled()
  })
})
