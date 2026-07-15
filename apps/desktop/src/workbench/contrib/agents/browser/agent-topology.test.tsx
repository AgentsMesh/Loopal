import { fireEvent, render, screen } from '@testing-library/react'
import { vi } from 'vitest'
import { type AgentSummary } from '../../../../shared/contracts'
import { AgentTopology } from './agent-topology'

describe('AgentTopology', () => {
  it('shows roots, descendants, missing parents, cycles, and bounded depth', () => {
    const agents: AgentSummary[] = [
      agent('root', 'Root'),
      agent('child', 'Child', 'root'),
      agent('orphan', 'Orphan', 'missing'),
      agent('cycle-a', 'Cycle A', 'cycle-b'),
      agent('cycle-b', 'Cycle B', 'cycle-a'),
      ...Array.from({ length: 10 }, (_, index) => (
        agent(`deep-${index}`, `Deep ${index}`, index === 0 ? 'root' : `deep-${index - 1}`)
      )),
    ]
    render(<AgentTopology agents={agents} />)

    expect(screen.getByLabelText('Agent topology')).toBeInTheDocument()
    expect(screen.getByText('Child').closest('[data-agent-id]')).toHaveAttribute(
      'data-parent-id', 'root',
    )
    expect(screen.getByText('Orphan').closest('[data-agent-id]')).toHaveAttribute(
      'data-parent-id', 'missing',
    )
    expect(screen.getByText('Cycle A')).toBeInTheDocument()
    expect(screen.getByText('Cycle B')).toBeInTheDocument()
    expect(screen.getByText('Deep 9')).toBeInTheDocument()
  })

  it('renders an explicit empty topology', () => {
    render(<AgentTopology agents={[]} />)
    expect(screen.getByText('No agents in this session.')).toBeInTheDocument()
  })

  it('marks unavailable nodes without preventing inspection', () => {
    render(<AgentTopology agents={[{
      ...agent('shadow', 'Shadow'), status: 'failed', controllable: false, error: 'spawn failed',
    }, {
      ...agent('remote', 'Remote'), status: 'starting', controllable: false,
      model: 'gpt-5', lastTool: 'Read',
    }]} />)
    const shadow = screen.getByRole('treeitem', { name: /Shadow/ })
    expect(shadow).toHaveTextContent('failed · retained')
    expect(shadow).toHaveTextContent('spawn failed')
    expect(shadow).toBeEnabled()
    expect(screen.getByRole('treeitem', { name: /Remote/ })).toHaveTextContent('unavailable')
  })

  it('selects an agent and exposes the selected state', () => {
    const onSelect = vi.fn()
    const { rerender } = render(
      <AgentTopology agents={[agent('root', 'Root')]} onSelect={onSelect} />,
    )
    fireEvent.click(screen.getByRole('treeitem', { name: /Root/ }))
    expect(onSelect).toHaveBeenCalledWith('root')
    rerender(
      <AgentTopology agents={[agent('root', 'Root')]} selectedAgentId="root" />,
    )
    expect(screen.getByRole('treeitem', { name: /Root/ })).toHaveClass('selected')
  })

  it('filters retained Agents and supports tree keyboard navigation', () => {
    render(<AgentTopology agents={[
      agent('root', 'Root'), agent('live', 'Live', 'root'),
      { ...agent('retained', 'Retained', 'root'), status: 'completed' },
    ]} />)
    expect(screen.queryByText('Retained')).not.toBeInTheDocument()
    expect(screen.getByText('2 active · 1 retained')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'All' }))
    const root = document.querySelector<HTMLButtonElement>('[data-agent-id="root"]')!
    root.focus()
    fireEvent.keyDown(root, { key: 'ArrowDown' })
    const live = document.querySelector<HTMLButtonElement>('[data-agent-id="live"]')!
    expect(live).toHaveFocus()
    fireEvent.keyDown(live, { key: 'End' })
    expect(document.querySelector('[data-agent-id="retained"]')).toHaveFocus()
  })
})

function agent(id: string, name: string, parentId?: string): AgentSummary {
  return { id, name, status: 'running', ...(parentId ? { parentId } : {}) }
}
