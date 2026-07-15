import { fireEvent, render, screen } from '@testing-library/react'
import { type LoopalSettingsValues } from '../../../../shared/contracts'
import { LoopalSettingsForm } from './loopal-settings-form'

const base: LoopalSettingsValues = {
  model: 'gpt-5', modelRouting: {
    default: '', summarization: 'summary-model', classification: '', refine: '',
  }, permissionMode: 'bypass', decisionMode: 'manual',
  sandboxPolicy: 'default_write', thinking: { type: 'budget', tokens: 2048 },
  maxContextTokens: 100_000, memoryEnabled: true, microcompactIdleMinutes: 60,
  telemetryEnabled: true, outputStyle: '',
}

describe('LoopalSettingsForm', () => {
  it('edits every persisted default without presenting it as live state', () => {
    const onChange = vi.fn()
    const view = render(<LoopalSettingsForm value={base} disabled={false} onChange={onChange} />)
    fireEvent.change(screen.getByLabelText('Default model'), { target: { value: 'next-model' } })
    fireEvent.change(screen.getByLabelText('Conversation override'), {
      target: { value: 'conversation-model' },
    })
    fireEvent.change(screen.getByLabelText('Summarization override'), { target: { value: '' } })
    fireEvent.change(screen.getByLabelText('Default permission mode'), {
      target: { value: 'ask_any_write' },
    })
    fireEvent.change(screen.getByLabelText('Default decision mode'), {
      target: { value: 'classifier' },
    })
    fireEvent.change(screen.getByLabelText('Default sandbox policy'), {
      target: { value: 'read_only' },
    })
    fireEvent.change(screen.getByLabelText('Thinking budget tokens'), { target: { value: '8192' } })
    fireEvent.change(screen.getByLabelText(/Max context tokens/), { target: { value: '200000' } })
    fireEvent.change(screen.getByLabelText(/Microcompact idle minutes/), { target: { value: '15' } })
    fireEvent.click(screen.getByLabelText('Enable project memory'))
    fireEvent.click(screen.getByLabelText('Enable telemetry'))
    fireEvent.change(screen.getByLabelText('Output style'), { target: { value: 'engineer' } })
    expect(onChange.mock.calls.map(([patch]) => patch)).toEqual([
      { model: 'next-model' },
      { modelRouting: { ...base.modelRouting, default: 'conversation-model' } },
      { modelRouting: { ...base.modelRouting, summarization: '' } },
      { permissionMode: 'ask_any_write' },
      { decisionMode: 'classifier' }, { sandboxPolicy: 'read_only' },
      { thinking: { type: 'budget', tokens: 8192 } }, { maxContextTokens: 200000 },
      { microcompactIdleMinutes: 15 }, { memoryEnabled: false },
      { telemetryEnabled: false }, { outputStyle: 'engineer' },
    ])
    fireEvent.change(screen.getByLabelText('Default thinking'), { target: { value: 'budget' } })
    expect(onChange).toHaveBeenCalledWith({ thinking: { type: 'budget', tokens: 2048 } })

    view.rerender(<LoopalSettingsForm value={{ ...base, thinking: { type: 'auto' } }}
      disabled={false} onChange={onChange} />)
    fireEvent.change(screen.getByLabelText('Default thinking'), { target: { value: 'budget' } })
    fireEvent.change(screen.getByLabelText('Default thinking'), { target: { value: 'effort:max' } })
    fireEvent.change(screen.getByLabelText('Default thinking'), { target: { value: 'disabled' } })
    fireEvent.change(screen.getByLabelText('Default thinking'), { target: { value: 'auto' } })
    expect(onChange).toHaveBeenCalledWith({ thinking: { type: 'budget', tokens: 4096 } })
    expect(onChange).toHaveBeenCalledWith({ thinking: { type: 'effort', level: 'max' } })
    expect(onChange).toHaveBeenCalledWith({ thinking: { type: 'disabled' } })
    expect(onChange).toHaveBeenCalledWith({ thinking: { type: 'auto' } })
    view.rerender(<LoopalSettingsForm value={{
      ...base, thinking: { type: 'effort', level: 'medium' },
    }} disabled={false} onChange={onChange} />)
    expect(screen.getByLabelText('Default thinking')).toHaveValue('effort:medium')
  })

  it('disables all controls while a save or restart is pending', () => {
    render(<LoopalSettingsForm value={base} disabled onChange={vi.fn()} />)
    const controls = ['textbox', 'combobox', 'spinbutton', 'checkbox'].flatMap(
      (role) => screen.queryAllByRole(role),
    )
    for (const control of controls) {
      expect(control).toBeDisabled()
    }
  })
})
