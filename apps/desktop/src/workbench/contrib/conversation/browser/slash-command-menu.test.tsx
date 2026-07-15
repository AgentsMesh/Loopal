import { fireEvent, render, screen } from '@testing-library/react'
import { type ComponentProps } from 'react'
import { MessageComposer } from './message-composer'
import { type SlashCommandItem } from './slash-command-model'

const commands: readonly SlashCommandItem[] = [
  command('/act', 'none', 'runtime'),
  command('/plan', 'none', 'runtime'),
  command('/permission', 'required', 'runtime'),
  command('/desktop-check', 'optional', 'skill'),
]

function command(
  name: string, argument: SlashCommandItem['arguments'], source: SlashCommandItem['source'],
): SlashCommandItem {
  return {
    name, arguments: argument, source, usage: name,
    description: `${name} description`, sourceLabel: source,
  }
}

function props(
  overrides: Partial<ComponentProps<typeof MessageComposer>> = {},
): ComponentProps<typeof MessageComposer> {
  const noop = (): void => undefined
  return {
    label: 'Message Loopal', placeholder: 'Ask Loopal', draft: '/', images: [],
    disabled: false, sending: false, canControl: true, hasSession: true,
    sessionLive: true, canRestartSession: true, lifecycleBusy: false,
    mode: 'act', agentName: 'Loopal', commands, onDraftChange: noop, onSend: noop,
    onSelectImages: noop, onRemoveImage: noop, onModeChange: noop,
    onStopSession: noop, onRestartSession: noop, ...overrides,
  }
}

describe('composer slash command menu', () => {
  it('offers an accessible keyboard list and executes a selected no-arg Runtime command', () => {
    const onExecuteCommand = vi.fn()
    render(<MessageComposer {...props({ draft: '/p', onExecuteCommand })} />)
    const input = screen.getByRole('combobox', { name: 'Message Loopal' })
    const menu = screen.getByTestId('command-menu')
    const plan = menu.querySelector('[data-command-name="/plan"]')
    const permission = menu.querySelector('[data-command-name="/permission"]')
    expect(input).toHaveAttribute('aria-expanded', 'true')
    expect(input).toHaveAttribute('aria-controls', menu.id)
    expect(plan).toHaveAttribute('aria-selected', 'true')
    fireEvent.keyDown(input, { key: 'ArrowUp' })
    expect(permission).toHaveAttribute('aria-selected', 'true')
    fireEvent.keyDown(input, { key: 'ArrowDown' })
    expect(plan).toHaveAttribute('aria-selected', 'true')
    fireEvent.keyDown(input, { key: 'ArrowDown' })
    expect(permission).toHaveAttribute('aria-selected', 'true')
    fireEvent.keyDown(input, { key: 'ArrowUp' })
    expect(plan).toHaveAttribute('aria-selected', 'true')
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(onExecuteCommand).toHaveBeenCalledWith('/plan')
    expect(input).toHaveAttribute('aria-expanded', 'false')
  })

  it('uses Tab to complete a dynamic Skill and leaves execution on the message plane', () => {
    const onDraftChange = vi.fn()
    const onExecuteCommand = vi.fn()
    render(<MessageComposer {...props({
      draft: '/desktop', onDraftChange, onExecuteCommand,
    })} />)
    fireEvent.keyDown(screen.getByRole('combobox', { name: 'Message Loopal' }), { key: 'Tab' })
    expect(onDraftChange).toHaveBeenCalledWith('/desktop-check ')
    expect(onExecuteCommand).not.toHaveBeenCalled()
  })

  it('dismisses on Escape and does not submit during IME composition', () => {
    const onSend = vi.fn()
    const onExecuteCommand = vi.fn()
    const view = render(<MessageComposer {...props({ onSend, onExecuteCommand })} />)
    const input = screen.getByRole('combobox', { name: 'Message Loopal' })
    fireEvent.keyDown(input, { key: 'Escape' })
    expect(input).toHaveAttribute('aria-expanded', 'false')
    view.rerender(<MessageComposer {...props({
      draft: '/plan', onSend, onExecuteCommand,
    })} />)
    fireEvent.keyDown(input, { key: 'Enter', isComposing: true })
    expect(onSend).not.toHaveBeenCalled()
    expect(onExecuteCommand).not.toHaveBeenCalled()
  })

  it('submits a complete parameterized command instead of intercepting it as a menu choice', () => {
    const onSend = vi.fn()
    render(<MessageComposer {...props({ draft: '/permission bypass', onSend })} />)
    const input = screen.getByRole('combobox', { name: 'Message Loopal' })
    expect(input).toHaveAttribute('aria-expanded', 'false')
    fireEvent.keyDown(input, { key: 'Enter' })
    expect(onSend).toHaveBeenCalledOnce()
  })
})
