import { fireEvent, render, screen, within } from '@testing-library/react'
import { type LoopalBuiltInProviders } from '../../../../shared/contracts'
import { LoopalProviderSettings } from './loopal-provider-settings'

const empty = () => ({ enabled: false, baseUrl: '', apiKeyEnv: '', apiKeyConfigured: false })
const providers: LoopalBuiltInProviders = {
  anthropic: {
    enabled: true, baseUrl: 'https://api.example.test',
    apiKeyEnv: 'ANTHROPIC_API_KEY', apiKeyConfigured: true,
  },
  openai: empty(), google: empty(),
}

describe('LoopalProviderSettings', () => {
  it('keeps stored keys write-only and emits explicit provider patches', () => {
    const onChange = vi.fn()
    render(<LoopalProviderSettings providers={providers} updates={{}}
      disabled={false} onChange={onChange} />)
    const key = screen.getByLabelText('Anthropic API key')
    expect(key).toHaveValue('')
    expect(key).toHaveAttribute('type', 'password')
    expect(key).toHaveAttribute('placeholder', 'Configured')
    fireEvent.change(screen.getByLabelText('Anthropic base URL'), {
      target: { value: 'https://proxy.example.test/v1' },
    })
    fireEvent.change(screen.getByLabelText('Anthropic API key environment'), {
      target: { value: 'LOOPAL_ANTHROPIC_KEY' },
    })
    fireEvent.change(key, { target: { value: 'write-only-test-value' } })
    fireEvent.click(screen.getAllByRole('button', { name: 'Clear API key' })[0]!)
    fireEvent.click(screen.getAllByRole('button', { name: 'Remove local override' })[0]!)
    expect(onChange).toHaveBeenCalledWith('anthropic', {
      enabled: true, baseUrl: 'https://proxy.example.test/v1',
    })
    expect(onChange).toHaveBeenCalledWith('anthropic', {
      enabled: true, apiKeyEnv: 'LOOPAL_ANTHROPIC_KEY',
    })
    expect(onChange).toHaveBeenCalledWith('anthropic', {
      enabled: true, apiKey: 'write-only-test-value', clearApiKey: false,
    })
    expect(onChange).toHaveBeenCalledWith('anthropic', {
      apiKey: undefined, clearApiKey: true,
    })
    expect(onChange).toHaveBeenCalledWith('anthropic', { remove: true })
  })

  it('requires enabling a provider before editing its fields', () => {
    render(<LoopalProviderSettings providers={providers} updates={{}}
      disabled onChange={vi.fn()} />)
    for (const control of screen.getAllByRole('textbox')) expect(control).toBeDisabled()
    for (const control of screen.getAllByRole('checkbox')) expect(control).toBeDisabled()
  })

  it('projects pending enable, key, clear, remove, and undo updates', () => {
    const onChange = vi.fn()
    render(<LoopalProviderSettings providers={providers} updates={{
      anthropic: { remove: true },
      openai: { enabled: true, apiKey: 'pending-key' },
      google: { enabled: true, clearApiKey: true },
    }} disabled={false} onChange={onChange} />)

    expect(screen.getByLabelText('Enable Anthropic')).not.toBeChecked()
    expect(screen.getByLabelText('Anthropic base URL')).toBeDisabled()
    fireEvent.click(screen.getByRole('button', { name: 'Undo remove' }))
    expect(onChange).toHaveBeenCalledWith('anthropic', undefined)

    expect(screen.getByLabelText('Enable OpenAI')).toBeChecked()
    expect(screen.getByLabelText('OpenAI API key')).toHaveValue('pending-key')
    expect(screen.getByLabelText('OpenAI API key')).toHaveAttribute('placeholder', 'Configured')
    fireEvent.change(screen.getByLabelText('OpenAI API key'), { target: { value: '' } })
    expect(onChange).toHaveBeenCalledWith('openai', {
      enabled: true, apiKey: undefined, clearApiKey: false,
    })

    expect(screen.getByLabelText('Enable Google')).toBeChecked()
    expect(screen.getByLabelText('Google API key')).toHaveAttribute('placeholder', 'Not configured')
    fireEvent.click(within(screen.getByTestId('provider-google'))
      .getByRole('button', { name: 'Remove local override' }))
    expect(onChange).toHaveBeenCalledWith('google', { remove: true })
  })
})
