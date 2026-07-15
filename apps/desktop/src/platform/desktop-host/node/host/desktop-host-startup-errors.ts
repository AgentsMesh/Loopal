export class DesktopSessionCreationStateUnknownError extends Error {
  readonly code = 'desktop_session_creation_state_unknown'

  constructor(cause: unknown) {
    const detail = cause instanceof Error ? cause.message : String(cause)
    super(
      'desktop_session_creation_state_unknown: Host failed after ALIVE before Session identity; '
        + `original failure (${detail})`,
      { cause },
    )
  }
}

export function cleanupFailure(startup: unknown, cleanup: unknown): Error {
  return combinedFailure(startup, cleanup, 'cleanup failed')
}

export function activationFailure(startup: unknown, activation: unknown): Error {
  return combinedFailure(startup, activation, 'activation failed')
}

export function isTerminationUncertain(value: unknown): boolean {
  if (value instanceof AggregateError) return value.errors.some(isTerminationUncertain)
  if (!(value instanceof Error)) return false
  return value.message.includes('desktop_protocol_drain_incomplete')
    || value.message.includes('desktop_process_termination_unconfirmed')
    || isTerminationUncertain(value.cause)
}

function combinedFailure(primary: unknown, related: unknown, label: string): Error {
  const primaryMessage = primary instanceof Error ? primary.message : String(primary)
  const relatedMessage = related instanceof Error ? related.message : String(related)
  return new Error(`${primaryMessage}; ${label} (${relatedMessage})`, {
    cause: new AggregateError([primary, related]),
  })
}
