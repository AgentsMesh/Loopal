import { type SessionSummary } from '../../../../shared/contracts'
import { isSessionLive } from '../../../../shared/contracts/session-lifecycle'

export interface SessionCatalogModel {
  readonly currentSessions: readonly SessionSummary[]
  readonly searchResults: readonly SessionSummary[]
}

export function sessionCatalogModel(
  sessions: readonly SessionSummary[], query: string,
): SessionCatalogModel {
  const ordered = [...sessions].sort(compareRecent)
  const currentSessions = ordered.filter(isSessionLive)
  const normalized = query.trim().toLocaleLowerCase()
  const searchResults = normalized
    ? ordered.filter((session) => session.title.toLocaleLowerCase().includes(normalized))
    : []
  return { currentSessions, searchResults }
}

export function preferredSessionId(
  sessions: readonly SessionSummary[], activeSessionId?: string,
): string | undefined {
  if (activeSessionId) return activeSessionId
  return sessionCatalogModel(sessions, '').currentSessions[0]?.id
}

function compareRecent(left: SessionSummary, right: SessionSummary): number {
  const updated = Date.parse(right.updatedAt) - Date.parse(left.updatedAt)
  return updated || left.id.localeCompare(right.id)
}
