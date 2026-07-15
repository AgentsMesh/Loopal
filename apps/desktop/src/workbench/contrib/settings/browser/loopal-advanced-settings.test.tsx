import { fireEvent, render, screen } from '@testing-library/react'
import { type LoopalDefaultSettings } from '../../../../shared/contracts'
import { LoopalAdvancedSettings } from './loopal-advanced-settings'

const record = {
  workspaceId: 'workspace',
  settings: {
    model: 'gpt-5', modelRouting: {
      default: '', summarization: '', classification: '', refine: '',
    }, permissionMode: 'bypass', decisionMode: 'manual', sandboxPolicy: 'default_write',
    thinking: { type: 'auto' }, maxContextTokens: 0, memoryEnabled: true,
    microcompactIdleMinutes: 60, telemetryEnabled: true, outputStyle: '',
  },
  configuredProviders: [],
  providers: {
    anthropic: empty(), openai: empty(), google: empty(),
  },
  openaiCompatible: [],
  resolvedEntries: [
    { key: 'model', value: 'gpt-5' },
    { key: 'providers.anthropic.api_key', value: '********' },
  ],
  settingSources: ['project settings', 'project local overrides'],
} satisfies LoopalDefaultSettings

describe('LoopalAdvancedSettings', () => {
  it('renders a searchable read-only resolved projection with sources', () => {
    render(<LoopalAdvancedSettings record={record} />)
    fireEvent.click(screen.getByText('Advanced resolved config'))
    expect(screen.getByText(/project settings · project local overrides/)).toBeVisible()
    fireEvent.change(screen.getByLabelText('Search resolved config'), {
      target: { value: 'api_key' },
    })
    expect(screen.getByText('providers.anthropic.api_key')).toBeVisible()
    expect(screen.getByText('********')).toBeVisible()
    expect(screen.queryByText('gpt-5')).not.toBeInTheDocument()
  })
})

function empty() {
  return { enabled: false, baseUrl: '', apiKeyEnv: '', apiKeyConfigured: false }
}
