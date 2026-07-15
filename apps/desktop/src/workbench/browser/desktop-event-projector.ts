import {
  type DesktopEvent,
  type HostStatus,
  type RuntimeSummary,
  type SessionDetail,
  type SessionSummary,
} from '../../shared/contracts'

export interface DesktopProjection {
  readonly hostStatus: HostStatus
  readonly sessions: readonly SessionSummary[]
  readonly runtimes: readonly RuntimeSummary[]
  readonly detail?: SessionDetail
}

export const initialDesktopProjection: DesktopProjection = {
  hostStatus: 'spawning',
  sessions: [],
  runtimes: [],
}

export function projectDesktopEvent(
  current: DesktopProjection,
  event: DesktopEvent,
): DesktopProjection {
  switch (event.type) {
    case 'host_status':
      return { ...current, hostStatus: event.status }
    case 'session_updated':
      if (current.detail?.session.id !== event.session.id) return {
        ...current,
        sessions: replaceOrAppend(current.sessions, event.session),
      }
      return {
        ...current,
        sessions: replaceOrAppend(current.sessions, event.session),
        detail: { ...current.detail, session: event.session },
      }
    case 'runtime_updated':
      return { ...current, runtimes: projectRuntime(current.runtimes, event.runtime) }
    case 'session_detail_replaced':
      return current.detail?.session.id === event.detail.session.id
        ? {
            ...current,
            sessions: replaceOrAppend(current.sessions, event.detail.session),
            detail: event.detail,
          }
        : current
    case 'conversation_entry':
      return current.detail?.session.id === event.sessionId
        ? {
            ...current,
            detail: {
              ...current.detail,
              conversation: [...current.detail.conversation, event.entry],
            },
          }
        : current
    case 'artifact_created':
      return current.detail?.session.id === event.artifact.sessionId
        ? {
            ...current,
            detail: {
              ...current.detail,
              artifacts: replaceOrAppend(current.detail.artifacts, event.artifact),
            },
          }
        : current
    default:
      return current
  }
}

function projectRuntime(
  runtimes: readonly RuntimeSummary[],
  runtime: RuntimeSummary,
): readonly RuntimeSummary[] {
  const newer = runtimes.some((candidate) => candidate.sessionId === runtime.sessionId
    && candidate.generation > runtime.generation)
  if (newer && (runtime.state === 'stopped' || runtime.state === 'crashed')) return runtimes
  const retained = runtimes.filter((candidate) => candidate.sessionId !== runtime.sessionId
    || candidate.id === runtime.id
    || candidate.generation > runtime.generation
    || (candidate.state !== 'stopped' && candidate.state !== 'crashed'))
  return replaceOrAppend(retained, runtime)
}

function replaceOrAppend<T extends { readonly id: string }>(
  values: readonly T[],
  value: T,
): T[] {
  return values.some((item) => item.id === value.id)
    ? values.map((item) => item.id === value.id ? value : item)
    : [...values, value]
}
