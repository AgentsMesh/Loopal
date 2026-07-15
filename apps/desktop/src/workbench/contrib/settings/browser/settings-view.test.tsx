import { fireEvent, render, screen, within } from '@testing-library/react'
import { richAgent, richDetail, richView } from '../../../../../test/fixtures/workbench/rich-session'
import { createTestAPI } from '../../../../../test/support/workbench/api-stub'
import { DEFAULT_DESKTOP_PREFERENCES } from './desktop-preferences'
import { SessionSettingsView, SettingsView } from './settings-view'

vi.mock('./metahub-settings-section', () => ({
  MetaHubSettingsSection: (props: {
    readonly target?: { readonly runtimeId: string }
    readonly initialState?: { readonly state: string }
    readonly visible?: boolean
  }) => props.visible === false ? null
    : <div data-testid="metahub-slot" data-runtime={props.target?.runtimeId}
      data-state={props.initialState?.state} />,
}))

function renderSettings(
  overrides: Partial<Parameters<typeof SettingsView>[0]> = {}, includeDetail = true,
) {
  const onPreferences = vi.fn()
  const onControl = vi.fn()
  const onSelectAgent = vi.fn()
  const onInterrupt = vi.fn()
  const onClose = vi.fn()
  const detail = richDetail({
    agents: [richAgent(), richAgent({ id: 'child', name: 'Child', parentId: 'main' })],
    view: richView(),
  })
  const view = render(<SettingsView
    {...(includeDetail ? { detail } : {})} hostStatus="ready" selectedAgentId="main"
    onSelectAgent={onSelectAgent} canControl busy={false}
    preferences={DEFAULT_DESKTOP_PREFERENCES} onPreferences={onPreferences}
    onInterrupt={onInterrupt} onControl={onControl} onClose={onClose}
    metaHubSettings={<div>MetaHub settings</div>}
    {...overrides}
  />)
  return { onPreferences, onControl, onSelectAgent, onInterrupt, onClose, unmount: view.unmount }
}

describe('SettingsView', () => {
  it('applies real desktop, Agent, MCP, and selection controls', () => {
    const actions = renderSettings()
    fireEvent.change(screen.getByLabelText('Panel density'), { target: { value: 'compact' } })
    fireEvent.change(screen.getByLabelText('Conversation font size'), { target: { value: '16' } })
    fireEvent.click(screen.getByLabelText('Show agent topology'))
    expect(actions.onPreferences.mock.calls.map(([patch]) => patch)).toEqual([
      { panelDensity: 'compact' }, { conversationFontSize: 16 }, { showAgentTopology: false },
    ])
    fireEvent.click(screen.getByRole('tab', { name: 'Current Agent (live)' }))
    fireEvent.change(screen.getByLabelText('Settings agent'), { target: { value: 'child' } })
    expect(actions.onSelectAgent).toHaveBeenCalledWith('child')
    const controls = screen.getByRole('group', { name: 'Agent controls' })
    fireEvent.change(within(controls).getByLabelText('Agent mode'), { target: { value: 'plan' } })
    fireEvent.click(within(controls).getByRole('button', { name: 'Interrupt' }))
    fireEvent.click(screen.getByRole('tab', { name: 'Runtime and MCP' }))
    fireEvent.click(screen.getByRole('button', { name: 'Refresh MCP status' }))
    expect(actions.onControl).toHaveBeenNthCalledWith(1, { type: 'mode', mode: 'plan' })
    expect(actions.onControl).toHaveBeenNthCalledWith(2, { type: 'mcp_status' })
    expect(actions.onInterrupt).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: 'Close settings' }))
    expect(actions.onClose).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('tab', { name: 'MetaHub' }))
    expect(screen.getByText('MetaHub settings')).toBeInTheDocument()
  })

  it('renders selected child state and a safe no-session fallback', () => {
    const detail = richDetail({
      agents: [richAgent(), richAgent({
        id: 'child', name: 'Child', parentId: 'main', model: 'child-model', view: richView(),
      })],
    })
    const first = renderSettings({ detail, selectedAgentId: 'child', canControl: false, busy: true })
    fireEvent.click(screen.getByRole('tab', { name: 'Current Agent (live)' }))
    expect(screen.getByDisplayValue('Child · running')).toBeInTheDocument()
    expect(screen.getByDisplayValue('child-model')).toBeDisabled()
    fireEvent.click(screen.getByRole('tab', { name: 'Runtime and MCP' }))
    expect(screen.getByTestId('diagnostics-pane')).toHaveTextContent('child-model')
    expect(first.onControl).not.toHaveBeenCalled()

    first.unmount()
    renderSettings({ selectedAgentId: 'missing' }, false)
    fireEvent.click(screen.getByRole('tab', { name: 'Current Agent (live)' }))
    expect(screen.getByText('Select a live session to configure its Agent.')).toBeInTheDocument()
    expect(screen.queryByLabelText('Settings agent')).not.toBeInTheDocument()
  })

  it('binds Settings to the exact active MetaHub runtime generation', () => {
    const { api } = createTestAPI()
    const onClose = vi.fn()
    const detail = richDetail({
      metaHub: {
        state: 'connected', hubs: [], topology: [], refreshedAt: '2026-07-11T12:00:00.000Z',
      },
    })
    const view = render(<SessionSettingsView
      api={api} runtimes={[{
        id: 'runtime-rich', sessionId: detail.session.id, workspaceId: 'workspace',
        generation: 4, state: 'ready', rootAgent: 'main',
      }]}
      detail={detail} hostStatus="ready" selectedAgentId="main"
      onSelectAgent={vi.fn()} canControl busy={false}
      preferences={DEFAULT_DESKTOP_PREFERENCES} onPreferences={vi.fn()}
      onInterrupt={vi.fn()} onControl={vi.fn()} onClose={onClose}
    />)
    fireEvent.click(screen.getByRole('tab', { name: 'MetaHub' }))
    expect(screen.getByTestId('metahub-slot')).toHaveAttribute('data-runtime', 'runtime-rich')
    expect(screen.getByTestId('metahub-slot')).toHaveAttribute('data-state', 'connected')
    view.rerender(<SessionSettingsView
      api={api} runtimes={[]} hostStatus="stopped" selectedAgentId="missing"
      onSelectAgent={vi.fn()} canControl={false} busy
      preferences={DEFAULT_DESKTOP_PREFERENCES} onPreferences={vi.fn()}
      onInterrupt={vi.fn()} onControl={vi.fn()} onClose={onClose}
    />)
    expect(screen.getByTestId('metahub-slot')).not.toHaveAttribute('data-runtime')
  })

  it('wires Skills and Plugins to the selected workspace with a no-session fallback', async () => {
    const fallback = createTestAPI()
    const listSkills = vi.fn(fallback.api.listSkills)
    const listPlugins = vi.fn(fallback.api.listPlugins)
    const { api } = createTestAPI({ listSkills, listPlugins })
    const detail = richDetail({ agents: [richAgent()] })
    const first = render(<SessionSettingsView api={api} runtimes={[]} detail={detail}
      hostStatus="ready" selectedAgentId="main" onSelectAgent={vi.fn()}
      canControl busy={false} preferences={DEFAULT_DESKTOP_PREFERENCES}
      onPreferences={vi.fn()} onInterrupt={vi.fn()} onControl={vi.fn()} onClose={vi.fn()} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Skills & Plugins' }))
    expect(await screen.findByTestId('skills-plugin-settings')).toBeVisible()
    expect(listSkills).toHaveBeenCalledWith(detail.session.workspaceId)
    expect(listPlugins).toHaveBeenCalledWith(detail.session.workspaceId)
    first.unmount()

    render(<SessionSettingsView api={api} runtimes={[]} hostStatus="stopped"
      selectedAgentId="missing" onSelectAgent={vi.fn()} canControl={false} busy
      preferences={DEFAULT_DESKTOP_PREFERENCES} onPreferences={vi.fn()}
      onInterrupt={vi.fn()} onControl={vi.fn()} onClose={vi.fn()} />)
    fireEvent.click(screen.getByRole('tab', { name: 'Skills & Plugins' }))
    expect(screen.getByText('Open a live Session to inspect its project Skills and Plugins.'))
      .toBeInTheDocument()
  })

  it('keeps an unsaved Loopal draft while only mounting the selected section DOM', async () => {
    const fallback = createTestAPI()
    const getLoopalSettings = vi.fn(fallback.api.getLoopalSettings)
    const { api } = createTestAPI({ getLoopalSettings })
    const detail = richDetail({ agents: [richAgent()] })
    render(<SessionSettingsView api={api} runtimes={[]} detail={detail}
      hostStatus="ready" selectedAgentId="main" onSelectAgent={vi.fn()}
      canControl busy={false} preferences={DEFAULT_DESKTOP_PREFERENCES}
      onPreferences={vi.fn()} onInterrupt={vi.fn()} onControl={vi.fn()} onClose={vi.fn()} />)

    fireEvent.click(screen.getByRole('tab', { name: 'Defaults' }))
    const model = await screen.findByLabelText('Default model')
    fireEvent.change(model, { target: { value: 'unsaved-model' } })
    fireEvent.click(screen.getByRole('tab', { name: 'Desktop appearance' }))
    expect(screen.queryByLabelText('Default model')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('tab', { name: 'Defaults' }))
    expect(screen.getByDisplayValue('unsaved-model')).toBeVisible()
    expect(getLoopalSettings).toHaveBeenCalledOnce()
  })
})
