import {
  type HostStatus,
  type RuntimeSummary,
  type SessionSummary,
} from '../../../../shared/contracts'
import {
  type SessionRuntimeScope,
  type SessionRuntimeStatusEvent,
} from '../runtime/session-runtime-registry'

export function runtimeSummary(
  scope: SessionRuntimeScope,
  status: HostStatus,
  now: Date,
  startedAt?: string,
): RuntimeSummary {
  return {
    id: scope.runtimeId,
    sessionId: scope.sessionId,
    workspaceId: scope.workspaceId,
    generation: scope.generation,
    state: runtimeState(status),
    rootAgent: 'main',
    startedAt: startedAt ?? now.toISOString(),
  }
}

export function runtimeState(status: HostStatus): RuntimeSummary['state'] {
  if (status === 'ready') return 'ready'
  if (status === 'stopping') return 'stopping'
  if (status === 'stopped') return 'stopped'
  if (status === 'crashed') return 'crashed'
  return 'starting'
}

export function hostSession(
  session: SessionSummary,
  event: SessionRuntimeStatusEvent,
  updatedAt: string,
): SessionSummary {
  const { activeRuntimeId: _runtime, attention: _attention, ...base } = session
  if (event.status === 'crashed') {
    return { ...base, status: 'failed', attention: 'failure', updatedAt }
  }
  if (event.status === 'stopping' || event.status === 'stopped') {
    return { ...base, status: 'stopped', updatedAt }
  }
  return {
    ...base,
    status: event.status === 'ready' ? 'waiting' : 'starting',
    activeRuntimeId: event.runtimeId,
    updatedAt,
  }
}

export function runtimeFields(session: SessionSummary) {
  return {
    status: session.status,
    activeRuntimeId: session.activeRuntimeId,
    ...(session.attention ? { attention: session.attention } : {}),
  }
}
