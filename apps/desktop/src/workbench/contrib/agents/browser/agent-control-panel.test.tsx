import { fireEvent, render, screen } from '@testing-library/react'
import { type AgentSummary } from '../../../../shared/contracts'
import { AgentControlPanel } from './agent-control-panel'

const agent: AgentSummary = {
  id: 'main', name: 'Loopal', status: 'running', model: 'gpt-5',
  mode: 'act',
  thinkingConfig: 'auto', permissionMode: 'ask_dangerous',
  decisionMode: 'classifier', sandboxPolicy: 'default_write',
  telemetry: {
    turnCount: 4, inputTokens: 0, outputTokens: 0, cacheCreationTokens: 0,
    cacheReadTokens: 0, thinkingTokens: 0, contextWindow: 0,
    toolsInFlight: 0, toolCount: 0,
  },
}

describe('AgentControlPanel', () => {
  it('exposes every agent configuration command with structured values', () => {
    const onControl = vi.fn()
    const onInterrupt = vi.fn()
    const { rerender } = render(
      <AgentControlPanel
        agent={agent} disabled={false} busy={false}
        onInterrupt={onInterrupt} onControl={onControl}
      />,
    )
    expect(screen.getByLabelText('Rewind turn index')).toHaveValue(3)
    fireEvent.click(screen.getByRole('button', { name: 'Interrupt' }))
    fireEvent.click(screen.getByRole('button', { name: 'Suspend' }))
    fireEvent.click(screen.getByRole('button', { name: 'Clear' }))
    fireEvent.change(screen.getByLabelText('Agent mode'), { target: { value: 'plan' } })
    fireEvent.change(screen.getByLabelText('Agent model'), { target: { value: 'gpt-5.1' } })
    fireEvent.click(screen.getByRole('button', { name: 'Apply agent model' }))
    for (const [label, value] of ([
      ['Thinking configuration', 'high'],
      ['Permission mode', 'bypass'],
      ['Decision mode', 'manual'],
      ['Sandbox policy', 'read_only'],
    ] as const)) fireEvent.change(screen.getByLabelText(label), { target: { value } })
    fireEvent.change(screen.getByLabelText('Compact instructions'), {
      target: { value: 'Preserve tool results' },
    })
    fireEvent.click(screen.getByRole('button', { name: 'Compact' }))
    fireEvent.change(screen.getByLabelText('Rewind turn index'), { target: { value: '2' } })
    fireEvent.click(screen.getByRole('button', { name: 'Rewind' }))

    expect(onInterrupt).toHaveBeenCalledOnce()
    expect(onControl.mock.calls.map(([command]) => command)).toEqual([
      { type: 'suspend' }, { type: 'clear' }, { type: 'mode', mode: 'plan' },
      { type: 'model', model: 'gpt-5.1' },
      { type: 'thinking', config: { type: 'effort', level: 'high' } },
      { type: 'permission', mode: 'bypass' },
      { type: 'decision', mode: 'manual' },
      { type: 'sandbox', policy: 'read_only' },
      { type: 'compact', instructions: 'Preserve tool results' },
      { type: 'rewind', turnIndex: 2 },
    ])
    expect(screen.getAllByRole('button').every((button) => button.hasAttribute('aria-label'))).toBe(true)

    rerender(
      <AgentControlPanel
        agent={{ ...agent, status: 'suspended' }} disabled={false} busy={false}
        onInterrupt={onInterrupt} onControl={onControl}
      />,
    )
    fireEvent.click(screen.getByRole('button', { name: 'Unsuspend' }))
    expect(onControl).toHaveBeenLastCalledWith({ type: 'unsuspend' })
  })

  it('disables actions while unavailable or busy and rejects invalid rewind', () => {
    const onControl = vi.fn()
    const { rerender } = render(
      <AgentControlPanel
        agent={agent} disabled busy={false}
        onInterrupt={vi.fn()} onControl={onControl}
      />,
    )
    expect(screen.getAllByRole('button').every((button) => button.hasAttribute('disabled'))).toBe(true)
    rerender(
      <AgentControlPanel
        agent={agent} disabled={false} busy
        onInterrupt={vi.fn()} onControl={onControl}
      />,
    )
    expect(screen.getAllByRole('button').every((button) => button.hasAttribute('disabled'))).toBe(true)
    rerender(
      <AgentControlPanel
        agent={agent} disabled={false} busy={false}
        onInterrupt={vi.fn()} onControl={onControl}
      />,
    )
    fireEvent.change(screen.getByLabelText('Rewind turn index'), { target: { value: '-1' } })
    expect(screen.getByRole('button', { name: 'Rewind' })).toBeDisabled()

    rerender(
      <AgentControlPanel
        agent={{ ...agent, thinkingConfig: 'adaptive' }} disabled={false} busy={false}
        onInterrupt={vi.fn()} onControl={onControl}
      />,
    )
    expect(screen.getByLabelText('Thinking configuration')).toHaveValue('adaptive')
    expect(screen.getByRole('option', { name: 'Observed: adaptive' })).toBeDisabled()

    rerender(
      <AgentControlPanel
        agent={{ ...agent, decisionMode: 'agent' }} disabled={false} busy={false}
        onInterrupt={vi.fn()} onControl={onControl}
      />,
    )
    expect(screen.getByRole('option', {
      name: 'Unsupported: Agent mode (observed)',
    })).toBeDisabled()

    rerender(
      <AgentControlPanel
        agent={{
          ...agent,
          telemetry: { ...agent.telemetry!, turnCount: 0 },
        }}
        disabled={false} busy={false} onInterrupt={vi.fn()} onControl={onControl}
      />,
    )
    expect(screen.getByLabelText('Rewind turn index')).toHaveValue(0)
    expect(screen.getByRole('button', { name: 'Rewind' })).toBeDisabled()
  })
})
