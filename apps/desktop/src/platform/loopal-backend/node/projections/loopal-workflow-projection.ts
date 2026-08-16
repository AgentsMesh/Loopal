import {
  type WorkflowRunSummary, type WorkflowRuns,
} from '../../../../shared/contracts'

const MAX_RECENT_WORKFLOWS = 32

export type WorkflowProjectionResult =
  | { readonly kind: 'applied'; readonly workflows: WorkflowRuns }
  | { readonly kind: 'noop' }
  | {
    readonly kind: 'gap'
    readonly expectedRevision: number
    readonly actualRevision: number
  }

export function reduceWorkflowRun(
  current: WorkflowRuns,
  summary: WorkflowRunSummary,
): WorkflowProjectionResult {
  const existing = [...current.active, ...current.recent]
    .find((run) => run.id === summary.id)
  if (existing) {
    if (existing.revision >= summary.revision || terminal(existing.state)) {
      return { kind: 'noop' }
    }
    const expectedRevision = existing.revision + 1
    if (summary.revision !== expectedRevision) {
      return { kind: 'gap', expectedRevision, actualRevision: summary.revision }
    }
  }

  const active = current.active.filter((run) => run.id !== summary.id)
  const recent = current.recent.filter((run) => run.id !== summary.id)
  if (terminal(summary.state)) {
    recent.push(summary)
    recent.sort((left, right) => (
      Date.parse(right.updatedAt) - Date.parse(left.updatedAt) || compareIds(left.id, right.id)
    ))
    recent.splice(MAX_RECENT_WORKFLOWS)
  } else {
    active.push(summary)
    active.sort((left, right) => (
      Date.parse(left.createdAt) - Date.parse(right.createdAt) || compareIds(left.id, right.id)
    ))
  }
  return { kind: 'applied', workflows: { active, recent } }
}

function terminal(state: WorkflowRunSummary['state']): boolean {
  return state === 'succeeded' || state === 'failed' || state === 'cancelled'
}

function compareIds(left: string, right: string): number {
  return left < right ? -1 : left > right ? 1 : 0
}
