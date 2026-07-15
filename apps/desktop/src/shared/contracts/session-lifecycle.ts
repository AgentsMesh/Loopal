import { type SessionSummary } from './session-contracts'

const LIVE_STATUSES = new Set<SessionSummary['status']>([
  'starting', 'running', 'waiting', 'failed',
])

export function isSessionLive(session: SessionSummary): boolean {
  return Boolean(session.activeRuntimeId) && LIVE_STATUSES.has(session.status)
}

export function canRestartSession(session: SessionSummary): boolean {
  return session.status !== 'archived'
}
