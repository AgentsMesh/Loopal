import { fireEvent, render, screen, within } from '@testing-library/react'
import { LoopalCompatibleProviderSettings } from './loopal-compatible-provider-settings'

const provider = {
  name: 'local', baseUrl: 'https://compat.example.test/v1',
  apiKeyEnv: 'COMPAT_KEY', modelPrefix: 'local/', apiKeyConfigured: true,
}

describe('LoopalCompatibleProviderSettings', () => {
  it('keeps direct keys write-only and emits endpoint operations', () => {
    const onChange = vi.fn()
    render(<LoopalCompatibleProviderSettings providers={[provider]} updates={[]}
      disabled={false} onChange={onChange} />)
    const key = screen.getByLabelText('Compatible API key')
    expect(key).toHaveValue('')
    expect(key).toHaveAttribute('type', 'password')
    expect(key).toHaveAttribute('placeholder', 'Configured')
    fireEvent.change(screen.getByLabelText('Compatible model prefix'), {
      target: { value: 'desktop/' },
    })
    expect(onChange).toHaveBeenCalledWith([{ name: 'local', modelPrefix: 'desktop/' }])
    fireEvent.click(screen.getByRole('button', { name: 'Clear API key' }))
    expect(onChange).toHaveBeenCalledWith([{
      name: 'local', apiKey: undefined, clearApiKey: true,
    }])
    fireEvent.click(screen.getByRole('button', { name: 'Remove provider' }))
    expect(onChange).toHaveBeenCalledWith([{ name: 'local', remove: true }])
    fireEvent.click(screen.getByRole('button', { name: 'Add endpoint' }))
    expect(onChange).toHaveBeenCalledWith([{ name: 'compatible-1', baseUrl: '' }])
  })

  it('disables endpoint controls while settings are busy', () => {
    render(<LoopalCompatibleProviderSettings providers={[provider]} updates={[]}
      disabled onChange={vi.fn()} />)
    for (const control of screen.getAllByRole('textbox')) expect(control).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Add endpoint' })).toBeDisabled()
  })

  it('creates, renames, edits, removes, and restores endpoints', () => {
    const onChange = vi.fn()
    const compatibleOne = { ...provider, name: 'compatible-1', apiKeyConfigured: false }
    const updates = [
      { name: 'local', remove: true as const },
      { name: 'draft', baseUrl: '' },
    ]
    const { container, rerender } = render(<LoopalCompatibleProviderSettings
      providers={[provider, compatibleOne]} updates={updates}
      disabled={false} onChange={onChange} />)

    const removed = container.querySelector<HTMLElement>('[data-provider-name="local"]')!
    expect(within(removed).getByLabelText('Compatible base URL')).toBeDisabled()
    fireEvent.click(within(removed).getByRole('button', { name: 'Undo remove' }))
    expect(onChange).toHaveBeenCalledWith([{ name: 'draft', baseUrl: '' }])

    const draft = container.querySelector<HTMLElement>('[data-provider-name="draft"]')!
    expect(within(draft).getByLabelText('Compatible API key'))
      .toHaveAttribute('placeholder', 'Not configured')
    fireEvent.change(within(draft).getByLabelText('Compatible provider name'), {
      target: { value: 'renamed' },
    })
    fireEvent.change(within(draft).getByLabelText('Compatible base URL'), {
      target: { value: 'https://renamed.example.test/v1' },
    })
    fireEvent.change(within(draft).getByLabelText('Compatible API key environment'), {
      target: { value: 'RENAMED_KEY' },
    })
    fireEvent.change(within(draft).getByLabelText('Compatible API key'), {
      target: { value: 'temporary' },
    })
    fireEvent.click(within(draft).getByRole('button', { name: 'Remove provider' }))
    expect(onChange).toHaveBeenCalledWith(expect.arrayContaining([{
      name: 'renamed', baseUrl: '',
    }]))

    fireEvent.click(screen.getByRole('button', { name: 'Add endpoint' }))
    expect(onChange).toHaveBeenCalledWith(expect.arrayContaining([{
      name: 'compatible-2', baseUrl: '',
    }]))

    rerender(<LoopalCompatibleProviderSettings providers={[provider]} updates={[{
      name: 'local', baseUrl: 'https://next.example.test/v1', modelPrefix: 'next/',
      apiKeyEnv: 'NEXT_KEY', clearApiKey: true,
    }]} disabled={false} onChange={onChange} />)
    expect(screen.getByLabelText('Compatible base URL'))
      .toHaveValue('https://next.example.test/v1')
    expect(screen.getByLabelText('Compatible model prefix')).toHaveValue('next/')
    expect(screen.getByLabelText('Compatible API key environment')).toHaveValue('NEXT_KEY')
    expect(screen.getByRole('button', { name: 'Clear API key' })).toBeDisabled()
    fireEvent.change(screen.getByLabelText('Compatible API key'), { target: { value: '' } })
  })
})
