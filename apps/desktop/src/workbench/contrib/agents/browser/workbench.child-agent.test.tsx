import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { richView } from '../../../../../test/fixtures/workbench/rich-session'
import {
  createTestAPI, sessionDetail, sessionOne, updatedAt,
} from '../../../../../test/support/workbench/api-stub'
import { Workbench } from '../../../browser/workbench'

// Clicking an already-selected panel tab COLLAPSES the panel (toggle
// semantics), so a blind click races the zone's initial expansion state and
// can hide the tree it meant to open. Converge instead: each retry expands
// the panel if it is collapsed, then selects the agent node.
async function selectAgent(root: HTMLElement, name: RegExp): Promise<void> {
  await waitFor(() => {
    const tab = within(root).getByRole('tab', { name: 'Agents' })
    if (tab.getAttribute('aria-expanded') !== 'true') fireEvent.click(tab)
    fireEvent.click(within(root).getByRole('treeitem', { name }))
  }, { timeout: 15_000 })
}

describe('Workbench child agent selection', () => {
  it('routes messages and resources to a live child while retaining completed output', async () => {
    const detail = {
      ...sessionDetail(sessionOne),
      agents: [{
        id: 'agent-session-1', name: 'Loopal', status: 'running' as const,
        children: ['child'],
      }, {
        id: 'child', name: 'Research child', status: 'waiting' as const,
        parentId: 'agent-session-1', conversation: [],
        view: richView({
          compactBanner: 'Child context compacted.',
          goal: {
            id: 'child-goal', objective: 'Child-only goal', status: 'active',
            createdAt: updatedAt, updatedAt,
          },
        }),
      }],
    }
    const sendMessage = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({ openSession: async () => detail, sendMessage })
    const { container } = render(<Workbench api={api} />)
    await screen.findByText(`Conversation for ${sessionOne.title}`)
    fireEvent.change(screen.getByLabelText('Message Loopal'), {
      target: { value: 'Root-only draft' },
    })
    await selectAgent(container, /Research child/)
    expect(
      await screen.findByText('Child context compacted.', {}, { timeout: 15_000 }),
    ).toBeInTheDocument()
    expect(screen.getByText('Child-only goal')).toBeInTheDocument()
    const composer = screen.getByLabelText('Message Research child')
    expect(composer).toHaveValue('')
    fireEvent.change(composer, { target: { value: 'Report status' } })
    fireEvent.keyDown(composer, { key: 'Enter', isComposing: true })
    expect(sendMessage).not.toHaveBeenCalled()
    fireEvent.keyDown(composer, { key: 'Enter', shiftKey: false })
    await waitFor(() => expect(sendMessage).toHaveBeenCalledWith(
      sessionOne.id, 'Report status', 'child',
    ))
    replaceChild(events, detail, { status: 'starting' })
    expect(screen.getByLabelText('Message Research child')).toBeEnabled()
    replaceChild(events, detail, { status: 'waiting', controllable: false })
    expect(screen.getByLabelText('Message Research child')).toBeDisabled()
    replaceChild(events, detail, { status: 'completed' })
    const retained = screen.getByLabelText('Message Research child')
    expect(retained).toBeDisabled()
    expect(retained).toHaveAttribute('placeholder', expect.stringContaining('read-only'))
  })

  it('renders a child with no projected conversation or view', async () => {
    const detail = {
      ...sessionDetail(sessionOne),
      agents: [{
        id: 'agent-session-1', name: 'Loopal', status: 'running' as const,
        children: ['starting-child'],
      }, {
        id: 'starting-child', name: 'Starting child', status: 'starting' as const,
        parentId: 'agent-session-1', error: 'waiting for registration',
      }],
    }
    const { api } = createTestAPI({ openSession: async () => detail })
    const { container } = render(<Workbench api={api} />)
    await screen.findByText(`Conversation for ${sessionOne.title}`)
    await selectAgent(container, /Starting child/)
    await waitFor(() => expect(within(container).getByTestId('conversation')).toHaveTextContent(
      'Viewing Starting child · starting · waiting for registration',
    ), { timeout: 15_000 })
  })
})

function replaceChild(
  events: ReturnType<typeof createTestAPI>['events'],
  detail: Awaited<ReturnType<typeof sessionDetail>>,
  patch: { readonly status: 'starting' | 'waiting' | 'completed'; readonly controllable?: boolean },
): void {
  act(() => events.fire({
    type: 'session_detail_replaced',
    detail: {
      ...detail,
      agents: detail.agents.map((agent) => agent.id === 'child' ? { ...agent, ...patch } : agent),
    },
  }))
}
