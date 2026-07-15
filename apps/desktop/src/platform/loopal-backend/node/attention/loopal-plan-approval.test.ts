import { CancellationToken } from '../../../../base/common/cancellation'
import { PlanApprovalResponseInputSchema } from '../../../../shared/contracts'
import { projectAttentionEvent } from './loopal-attention'
import { FakeHost } from '../backend/loopal-backend.test-fixtures'
import { respondPlanApproval } from './loopal-code-attention'
import { type CodeWorkbenchRuntimeRouter } from '../workspace/loopal-code-workbench'

const now = () => new Date('2026-07-12T12:00:00.000Z')

describe('plan approval projection and routing', () => {
  it('projects the plan body and path into a scoped desktop request', () => {
    expect(projectAttentionEvent('plan_approval_requested', {
      id: 'plan-1', plan_content: '# Plan', plan_path: '/tmp/plan.md',
    }, {
      sessionId: 'session', workspaceId: 'workspace', runtimeId: 'runtime', generation: 2,
    }, 'main', now)).toEqual({
      type: 'plan_approval_requested',
      request: {
        id: 'plan-1', sessionId: 'session', runtimeId: 'runtime', generation: 2,
        agentId: 'main', planContent: '# Plan', planPath: '/tmp/plan.md',
        createdAt: '2026-07-12T12:00:00.000Z',
      },
    })
  })

  it('routes edited approval only through the matching live generation', async () => {
    const host = new FakeHost('session', [])
    host.request.mockResolvedValue({ resolved: true })
    const runtime = {
      sessionId: 'session', workspaceId: 'workspace', runtimeId: 'runtime', generation: 2,
      host,
    }
    const router: CodeWorkbenchRuntimeRouter = {
      workspace: vi.fn(async () => runtime),
      liveSession: vi.fn(async () => runtime),
    }
    const input = PlanApprovalResponseInputSchema.parse({
      sessionId: 'session', runtimeId: 'runtime', generation: 2,
      agentId: 'main', requestId: 'plan-1',
      decision: 'approve_with_edits', editedPlan: '# Edited',
    })
    await respondPlanApproval(router, input, CancellationToken.None)
    expect(host.request).toHaveBeenCalledWith('hub/plan_approval_response', {
      agent_name: 'main', request_id: 'plan-1',
      decision: 'approve_with_edits', edited_plan: '# Edited',
    }, expect.any(AbortSignal))
    await expect(respondPlanApproval(router, {
      ...input, generation: 3,
    }, CancellationToken.None)).rejects.toMatchObject({ code: 'RUNTIME_GONE' })
    expect(() => PlanApprovalResponseInputSchema.parse({
      ...input, editedPlan: undefined,
    })).toThrow('Edited plan is required')
  })
})
