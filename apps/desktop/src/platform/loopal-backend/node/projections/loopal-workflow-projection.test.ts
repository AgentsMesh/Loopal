import { type WorkflowRunSummary } from '../../../../shared/contracts'
import { reduceWorkflowRun } from './loopal-workflow-projection'

describe('workflow projection', () => {
  it('applies contiguous updates, moves terminal runs to recent, and drops stale updates', () => {
    const initial = { active: [run(1, 'running')], recent: [] }
    const stale = reduceWorkflowRun(initial, run(1, 'running'))
    expect(stale.kind).toBe('noop')

    const terminal = reduceWorkflowRun(initial, run(2, 'succeeded'))
    expect(terminal).toEqual({
      kind: 'applied',
      workflows: { active: [], recent: [run(2, 'succeeded')] },
    })
    if (terminal.kind !== 'applied') throw new Error('expected terminal update')
    expect(reduceWorkflowRun(terminal.workflows, run(3, 'failed')).kind).toBe('noop')
  })

  it('rejects revision gaps without mutating the current projection', () => {
    const current = { active: [run(2, 'running')], recent: [] }
    expect(reduceWorkflowRun(current, run(4, 'running'))).toEqual({
      kind: 'gap', expectedRevision: 3, actualRevision: 4,
    })
    expect(current.active[0]?.revision).toBe(2)
  })
})

function run(
  revision: number,
  state: WorkflowRunSummary['state'],
): WorkflowRunSummary {
  return {
    id: 'wrun_test', runGoal: 'Verify projections', state, revision, outputNode: 'done',
    counts: {
      pending: 0, ready: 0, active: state === 'running' ? 1 : 0,
      succeeded: state === 'succeeded' ? 1 : 0,
      failed: state === 'failed' ? 1 : 0, cancelled: 0, skipped: 0,
    },
    createdAt: '2026-07-11T12:00:00.000Z',
    updatedAt: `2026-07-11T12:00:0${Math.min(revision, 9)}.000Z`,
  }
}
