import { describe, expect, it, vi } from 'vitest'
import { LoopalEventProjector } from './loopal-event-projector'

const now = () => new Date('2026-07-11T12:00:00.000Z')

describe('LoopalEventProjector workflows', () => {
  it('projects valid root changes and rejects child or malformed changes', () => {
    const workflow = vi.fn()
    const projector = new LoopalEventProjector(now, {
      append: vi.fn(), updateSession: vi.fn(), attention: vi.fn(), workflow,
    })
    projector.finishSync(0, { worker: 0 })
    projector.accept(wire({ WorkflowRunChanged: workflowWire(1) }, 1))
    projector.accept(wire({ WorkflowRunChanged: workflowWire(1) }, 1))
    projector.accept(wire({ WorkflowRunChanged: workflowWire(2) }, 1, {
      hub: [], agent: 'worker',
    }))
    projector.accept(wire({ WorkflowRunChanged: { ...workflowWire(2), id: 'bad id' } }, 2))
    projector.accept(wire({ WorkflowRunChanged: { id: 'broken' } }, 3))
    expect(workflow).toHaveBeenCalledOnce()
    expect(workflow).toHaveBeenCalledWith(expect.objectContaining({
      id: 'wrun_test', runGoal: 'Verify projections', state: 'running', revision: 1,
    }))
  })
})

function wire(payload: unknown, revision: number, address: unknown = { hub: [], agent: 'main' }) {
  return {
    agent_name: address, event_id: 7, turn_id: 1, correlation_id: 2,
    rev: revision, payload,
  }
}

function workflowWire(revision: number) {
  return {
    id: 'wrun_test', run_goal: 'Verify projections', state: 'running', revision,
    output_node: 'done', created_at_unix_ms: 1_700_000_000_000,
    updated_at_unix_ms: 1_700_000_000_100 + revision,
    counts: {
      pending: 1, ready: 0, active: 1, succeeded: 0,
      failed: 0, cancelled: 0, skipped: 0,
    },
  }
}
