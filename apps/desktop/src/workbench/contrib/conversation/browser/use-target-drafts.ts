import { useState } from 'react'

export function useTargetDrafts(sessionId: string | undefined): {
  readonly get: (agentId?: string) => string
  readonly set: (agentId: string | undefined, value: string) => void
} {
  const [drafts, setDrafts] = useState<Record<string, string>>({})
  const key = (agentId?: string): string => (
    `${sessionId ?? '__no_session__'}\u0000${agentId ?? 'main'}`
  )
  return {
    get: (agentId) => drafts[key(agentId)] ?? '',
    set: (agentId, value) => setDrafts((current) => ({
      ...current, [key(agentId)]: value,
    })),
  }
}
