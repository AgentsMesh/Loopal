import { type AgentSummary } from '../../../../shared/contracts'

type Status = AgentSummary['status']

// Mirrors loopal-view-state's observable lifecycle reducer. Live events must
// update the cached AgentSummary immediately; snapshots are reconciliation,
// not the only owner of lifecycle state.
export function reduceAgentStatus(current: Status, kind: string, value: unknown): Status {
  if (kind === 'Running' || kind === 'Started') return 'running'
  if (kind === 'AwaitingInput' || kind === 'Interrupted' || kind === 'TurnCancelled') {
    return 'waiting'
  }
  if (kind === 'Error') return 'failed'
  if (kind === 'Finished') return current === 'failed' ? current : 'completed'
  if (kind !== 'ContinuationGateChanged' || !isRecord(value)) return current
  if (value.open === false && value.closed_reason === 'user_suspend') return 'suspended'
  if (value.open === true && current === 'suspended') return 'running'
  return current
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
