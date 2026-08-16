import { describe, expect, it, vi } from 'vitest'
import {
  liveSessionEvent as event, liveSessionHarness as harness,
} from './loopal-live-session.test-fixtures'

describe('LoopalLiveSession workflows', () => {
  it('publishes changes and replaces a revision gap from the authoritative snapshot', async () => {
    const value = harness()
    await value.state.initialize()
    const implementation = value.request.getMockImplementation()!
    value.request.mockImplementation(async (method, params, signal) => {
      if (method === 'view/snapshot') return {
        rev: 5,
        state: {
          agent: {
            name: 'main', observable: { status: 'WaitingForInput' },
            conversation: { streaming_text: '', messages: [] },
          },
          workflows: { active: [workflow(4)], recent: [] },
        },
      }
      return implementation(method, params, signal)
    })

    value.state.accept('agent/event', event({ WorkflowRunChanged: workflow(1) }, 3))
    expect(value.state.detail.view?.workflows.active[0]).toMatchObject({
      id: 'wrun_live', revision: 1, runGoal: 'Observe live workflow',
    })
    expect(value.events).toContainEqual(expect.objectContaining({
      type: 'session_detail_replaced',
    }))

    value.state.accept('agent/event', event({ WorkflowRunChanged: workflow(3) }, 4))
    expect(value.state.detail.view?.workflows.active[0]?.revision).toBe(1)
    await vi.waitFor(() => {
      expect(value.state.detail.view?.workflows.active[0]?.revision).toBe(4)
    })
    expect(value.request.mock.calls.filter(([method]) => method === 'view/snapshot').length)
      .toBeGreaterThanOrEqual(2)
  })
})

function workflow(revision: number) {
  return {
    id: 'wrun_live', run_goal: 'Observe live workflow', state: 'running', revision,
    output_node: 'done', created_at_unix_ms: 1_700_000_000_000,
    updated_at_unix_ms: 1_700_000_000_000 + revision,
    counts: {
      pending: 0, ready: 0, active: 1, succeeded: 0,
      failed: 0, cancelled: 0, skipped: 0,
    },
  }
}
