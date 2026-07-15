import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { createTestAPI } from '../../../../../test/support/workbench/api-stub'
import { type McpServerDefinition, type McpServersResponse } from '../../../../shared/contracts'
import { LoopalMcpSettings } from './loopal-mcp-settings'

const stdio: McpServerDefinition = {
  type: 'stdio', name: 'local', source: 'local', command: 'node', args: ['server.js'],
  enabled: true, timeoutMs: 30_000, sharing: 'hub-singleton', cwdIsolation: null,
  env: [{ name: 'TOKEN', configured: true }],
}
const http: McpServerDefinition = {
  type: 'streamable-http', name: 'remote', source: 'project',
  url: 'https://example.test/mcp', enabled: false, timeoutMs: 12_000,
  sharing: 'per-agent', headers: [],
}
const initial: McpServersResponse = { workspaceId: 'workspace', servers: [stdio, http] }

describe('LoopalMcpSettings', () => {
  it('renders the no-workspace and empty-workspace states', async () => {
    const list = vi.fn(async () => ({ workspaceId: 'workspace', servers: [] }))
    const { api } = createTestAPI({ listMcpServers: list })
    const view = render(<LoopalMcpSettings api={api} />)
    expect(screen.getByText(/Open a live Session/)).toBeVisible()
    expect(list).not.toHaveBeenCalled()
    view.rerender(<LoopalMcpSettings api={api} workspaceId="workspace" />)
    expect(screen.getByText('Loading MCP definitions…')).toBeVisible()
    expect(await screen.findByText('No MCP servers configured.')).toBeVisible()
  })

  it('adds, saves, edits, cancels, and deletes typed definitions', async () => {
    const list = vi.fn(async () => initial)
    const created: McpServerDefinition = {
      ...stdio, name: 'created', command: 'bun', env: [],
    }
    const upsert = vi.fn(async () => ({
      workspaceId: 'workspace', servers: [...initial.servers, created],
    }))
    const remove = vi.fn(async () => ({
      workspaceId: 'workspace', servers: [stdio, created],
    }))
    const { api } = createTestAPI({
      listMcpServers: list, upsertMcpServer: upsert, deleteMcpServer: remove,
    })
    render(<LoopalMcpSettings api={api} workspaceId="workspace" />)
    expect(await screen.findByText('local')).toBeVisible()
    expect(screen.getByText('remote')).toBeVisible()
    expect(screen.getByText('Enabled')).toBeVisible()
    expect(screen.getByText('Disabled')).toBeVisible()
    expect(screen.getByText('node')).toBeVisible()
    expect(screen.getByText('https://example.test/mcp')).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: 'Add MCP server' }))
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(screen.getByRole('button', { name: 'Add MCP server' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: 'Add MCP server' }))
    fireEvent.change(screen.getByLabelText('MCP server name'), { target: { value: 'created' } })
    fireEvent.change(screen.getByLabelText('MCP command'), { target: { value: 'bun' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save MCP server' }))
    await waitFor(() => expect(upsert).toHaveBeenCalledWith({
      workspaceId: 'workspace', server: expect.objectContaining({
        type: 'stdio', name: 'created', command: 'bun',
      }),
    }))
    expect(await screen.findByRole('status')).toHaveTextContent('Saved')
    const createdCard = screen.getByText('created').closest('article')!
    fireEvent.click(within(createdCard).getByRole('button', { name: 'Edit' }))
    expect(screen.getByLabelText('MCP server name')).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))

    const remoteCard = screen.getByText('remote').closest('article')!
    fireEvent.click(within(remoteCard).getByRole('button', {
      name: 'Delete MCP server remote',
    }))
    await waitFor(() => expect(remove).toHaveBeenCalledWith({
      workspaceId: 'workspace', name: 'remote',
    }))
    expect(await screen.findByRole('status')).toHaveTextContent('Deleted remote')
    expect(screen.queryByText('remote')).not.toBeInTheDocument()
  })

  it('surfaces load, validation, save, and delete failures', async () => {
    const failedLoad = createTestAPI({
      listMcpServers: async () => Promise.reject('plain load failure'),
    }).api
    const first = render(<LoopalMcpSettings api={failedLoad} workspaceId="workspace" />)
    expect(await screen.findByRole('alert')).toHaveTextContent('plain load failure')
    first.unmount()

    const upsert = vi.fn(async () => { throw new Error('save denied') })
    const remove = vi.fn(async () => { throw new Error('delete denied') })
    const { api } = createTestAPI({
      listMcpServers: async () => initial, upsertMcpServer: upsert,
      deleteMcpServer: remove,
    })
    render(<LoopalMcpSettings api={api} workspaceId="workspace" />)
    await screen.findByText('local')
    fireEvent.click(screen.getByRole('button', { name: 'Add MCP server' }))
    fireEvent.click(screen.getByRole('button', { name: 'Save MCP server' }))
    expect(await screen.findByRole('alert')).toBeVisible()
    expect(upsert).not.toHaveBeenCalled()
    fireEvent.change(screen.getByLabelText('MCP server name'), { target: { value: 'safe' } })
    fireEvent.change(screen.getByLabelText('MCP command'), { target: { value: 'node' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save MCP server' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('save denied')
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    const card = screen.getByText('local').closest('article')!
    fireEvent.click(within(card).getByRole('button', { name: 'Delete MCP server local' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('delete denied')
  })

  it('ignores a stale list response after workspace cleanup', async () => {
    let resolve!: (value: McpServersResponse) => void
    const list = vi.fn(() => new Promise<McpServersResponse>((accept) => { resolve = accept }))
    const { api } = createTestAPI({ listMcpServers: list })
    const view = render(<LoopalMcpSettings api={api} workspaceId="workspace" />)
    await waitFor(() => expect(list).toHaveBeenCalled())
    view.rerender(<LoopalMcpSettings api={api} />)
    resolve(initial)
    await Promise.resolve()
    expect(screen.getByText(/Open a live Session/)).toBeVisible()
    expect(screen.queryByRole('button', { name: 'Add MCP server' })).not.toBeInTheDocument()
  })
})
