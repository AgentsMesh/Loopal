import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { createTestAPI, updatedAt } from '../../../../../test/support/workbench/api-stub'
import { Workbench } from '../../../browser/workbench'

describe('plan approval attention', () => {
  it('renders plan details and sends approve, reject, and edited approval', async () => {
    const respondPlanApproval = vi.fn(async () => undefined)
    const { api, events } = createTestAPI({ respondPlanApproval })
    render(<Workbench api={api} />)
    await screen.findByText('Conversation for Build the desktop workbench')

    const request = (id: string, content = '# Plan\nStep 1') => ({
      type: 'plan_approval_requested' as const,
      request: {
        id, sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
        agentId: 'main', planContent: content, planPath: '/workspace/.loopal/plan.md',
        createdAt: updatedAt,
      },
    })
    const resolve = (id: string) => ({
      type: 'plan_approval_resolved' as const,
      sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
      agentId: 'main', requestId: id,
    })

    act(() => events.fire(request('approve')))
    let pane = await screen.findByTestId('plan-approvals-pane')
    expect(within(pane).getByTestId('plan-approval-path'))
      .toHaveTextContent('/workspace/.loopal/plan.md')
    expect(within(pane).getByTestId('plan-approval-content')).toHaveTextContent('Step 1')
    expect(within(pane).getByTestId('plan-approval-editor')).toHaveValue('# Plan\nStep 1')
    fireEvent.click(within(pane).getByTestId('plan-approval-approve'))
    await waitFor(() => expect(respondPlanApproval).toHaveBeenLastCalledWith({
      sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
      agentId: 'main', requestId: 'approve', decision: 'approve',
    }))

    act(() => {
      events.fire(resolve('approve'))
      events.fire(request('reject'))
    })
    pane = await screen.findByTestId('plan-approvals-pane')
    fireEvent.click(within(pane).getByTestId('plan-approval-reject'))
    await waitFor(() => expect(respondPlanApproval).toHaveBeenLastCalledWith({
      sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
      agentId: 'main', requestId: 'reject', decision: 'reject',
    }))

    act(() => {
      events.fire(resolve('reject'))
      events.fire(request('edits'))
    })
    pane = await screen.findByTestId('plan-approvals-pane')
    fireEvent.change(within(pane).getByTestId('plan-approval-editor'), {
      target: { value: '# Edited plan\nStep 2' },
    })
    fireEvent.click(within(pane).getByTestId('plan-approval-approve-edits'))
    await waitFor(() => expect(respondPlanApproval).toHaveBeenLastCalledWith({
      sessionId: 'session-1', runtimeId: 'runtime-1', generation: 1,
      agentId: 'main', requestId: 'edits', decision: 'approve_with_edits',
      editedPlan: '# Edited plan\nStep 2',
    }))
    act(() => events.fire(resolve('edits')))
    await waitFor(() => expect(screen.queryByTestId('plan-approvals-pane')).not.toBeInTheDocument())
  })
})
