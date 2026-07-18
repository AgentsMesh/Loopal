import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { createTestAPI } from '../../../../../test/support/workbench/api-stub'
import {
  type LoopalDefaultSettings as Settings, type LoopalDesktopAPI,
} from '../../../../shared/contracts'
import { LoopalDefaultSettings } from './loopal-default-settings'

const initial: Settings = {
  workspaceId: 'workspace',
  settings: {
    model: 'gpt-5', modelRouting: {
      default: '', summarization: '', classification: '', refine: '',
    }, permissionMode: 'bypass', decisionMode: 'manual',
    sandboxPolicy: 'default_write', thinking: { type: 'auto' }, maxContextTokens: 0,
    memoryEnabled: true, microcompactIdleMinutes: 60,
    telemetryEnabled: true, outputStyle: '',
  },
  configuredProviders: ['anthropic'],
  providers: {
    anthropic: { enabled: true, baseUrl: '', apiKeyEnv: '', apiKeyConfigured: true },
    openai: emptyProvider(), google: emptyProvider(),
  },
  openaiCompatible: [],
  resolvedEntries: [
    { key: 'model', value: 'gpt-5' },
    { key: 'providers.anthropic.api_key', value: '********' },
  ],
  settingSources: ['project local overrides'],
}

describe('LoopalDefaultSettings', () => {
  it('loads, atomically saves, and only then offers a Session restart', async () => {
    const get = vi.fn(async () => initial)
    const update = vi.fn<LoopalDesktopAPI['updateLoopalSettings']>(async (input) => ({
      workspaceId: input.workspaceId, settings: input.settings,
      configuredProviders: ['anthropic'], providers: initial.providers,
      openaiCompatible: initial.openaiCompatible,
      resolvedEntries: initial.resolvedEntries, settingSources: initial.settingSources,
    }))
    const restart = vi.fn(async () => ({
      id: 'runtime-2', sessionId: 'session', workspaceId: 'workspace', generation: 2,
      state: 'ready' as const, rootAgent: 'main',
    }))
    const { api } = createTestAPI({
      getLoopalSettings: get, updateLoopalSettings: update, restartSession: restart,
    })
    render(<LoopalDefaultSettings api={api} workspaceId="workspace" sessionId="session" />)
    expect(await screen.findByDisplayValue('gpt-5')).toBeVisible()
    expect(screen.getByTestId('configured-providers')).toHaveTextContent(
      'anthropic',
    )
    const save = screen.getByRole('button', { name: 'Save Loopal defaults' })
    const restartButton = screen.getByRole('button', { name: 'Restart current Session' })
    expect(save).toBeDisabled()
    fireEvent.change(screen.getByLabelText('Default model'), { target: { value: 'gpt-5.5' } })
    expect(save).toBeEnabled()
    expect(restartButton).toBeDisabled()
    fireEvent.click(save)
    await waitFor(() => expect(update).toHaveBeenCalledWith({
      workspaceId: 'workspace', settings: { ...initial.settings, model: 'gpt-5.5' },
    }))
    expect(await screen.findByRole('status')).toHaveTextContent('new or restarted Sessions')
    expect(restartButton).toBeEnabled()
    fireEvent.click(restartButton)
    await waitFor(() => expect(restart).toHaveBeenCalledWith('session'))
    // The status line updates only after the restart promise resolves and
    // commits — waiting on the mock call alone races that commit.
    await waitFor(() => expect(screen.getByRole('status'))
      .toHaveTextContent('restarted with the saved'))
  })

  it('surfaces load, validation, save, and restart failures without leaking credentials', async () => {
    const load = createTestAPI({
      getLoopalSettings: async () => { throw new Error('load denied') },
    }).api
    const view = render(<LoopalDefaultSettings api={load} workspaceId="workspace" />)
    expect(await screen.findByRole('alert')).toHaveTextContent('load denied')
    view.unmount()

    const update = vi.fn<LoopalDesktopAPI['updateLoopalSettings']>(async () => {
      throw new Error('save denied')
    })
    const restart = vi.fn(async () => { throw new Error('restart denied') })
    const { api } = createTestAPI({
      getLoopalSettings: async () => initial, updateLoopalSettings: update,
      restartSession: restart,
    })
    render(<LoopalDefaultSettings api={api} workspaceId="workspace" sessionId="session" />)
    const model = await screen.findByLabelText('Default model')
    fireEvent.change(model, { target: { value: '   ' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save Loopal defaults' }))
    expect(await screen.findByRole('alert')).toBeVisible()
    expect(update).not.toHaveBeenCalled()
    fireEvent.change(model, { target: { value: 'safe-model' } })
    fireEvent.click(screen.getByRole('button', { name: 'Save Loopal defaults' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('save denied')

    update.mockImplementation(async (input) => ({
      workspaceId: input.workspaceId, settings: input.settings, configuredProviders: [],
      providers: initial.providers, resolvedEntries: [], settingSources: ['defaults'],
      openaiCompatible: [],
    }))
    fireEvent.click(screen.getByRole('button', { name: 'Save Loopal defaults' }))
    await waitFor(() => expect(screen.getByRole('button', {
      name: 'Restart current Session',
    })).toBeEnabled())
    fireEvent.click(screen.getByRole('button', { name: 'Restart current Session' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('restart denied')
    expect(document.body).not.toHaveTextContent('api_key')
  })

  it('renders a safe no-workspace fallback without crossing preload', () => {
    const get = vi.fn()
    const { api } = createTestAPI({ getLoopalSettings: get })
    render(<LoopalDefaultSettings api={api} />)
    expect(screen.getByText(/Open a live Session/)).toBeVisible()
    expect(get).not.toHaveBeenCalled()
  })

  it('normalizes non-Error Host failures', async () => {
    const { api } = createTestAPI({
      getLoopalSettings: async () => Promise.reject('plain failure'),
    })
    render(<LoopalDefaultSettings api={api} workspaceId="workspace" />)
    expect(await screen.findByRole('alert')).toHaveTextContent('plain failure')
  })
})

function emptyProvider() {
  return { enabled: false, baseUrl: '', apiKeyEnv: '', apiKeyConfigured: false }
}
