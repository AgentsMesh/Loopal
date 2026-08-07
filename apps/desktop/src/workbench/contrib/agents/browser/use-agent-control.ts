import { useState } from 'react'
import {
  type AgentControlCommand,
  type AgentControlDisposition,
  type AgentControlTarget,
  type LoopalDesktopAPI,
} from '../../../../shared/contracts'
import { type DesktopProjection } from '../../../browser/desktop-event-projector'

export function useAgentControl(
  api: LoopalDesktopAPI,
  projection: DesktopProjection,
) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const [disposition, setDisposition] = useState<AgentControlDisposition>()

  const target = (agentId: string): AgentControlTarget | undefined => {
    const detail = projection.detail
    const runtimeId = detail?.session.activeRuntimeId
    const runtime = projection.runtimes.find((candidate) => candidate.id === runtimeId)
    const agent = detail?.agents.find((candidate) => candidate.id === agentId)
    if (!detail || !runtime || runtime.sessionId !== detail.session.id
      || !agent || !['starting', 'ready'].includes(runtime.state)
      || agent.controllable === false
      || agent.status === 'completed' || agent.status === 'failed') return undefined
    return {
      sessionId: detail.session.id,
      runtimeId: runtime.id,
      generation: runtime.generation,
      agentId,
    }
  }

  const run = async (
    agentId: string,
    command?: AgentControlCommand,
  ): Promise<boolean> => {
    if (busy) return false
    const selected = target(agentId)
    if (!selected) {
      setError('The selected agent no longer has a live runtime.')
      return false
    }
    setBusy(true)
    setError(undefined)
    setDisposition(undefined)
    try {
      if (command) {
        const next = await api.controlAgent({ target: selected, command })
        setDisposition(next)
        if (next.status === 'rejected') {
          setError(next.reason)
          return false
        }
      } else await api.interruptAgent(selected)
      return true
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
      return false
    } finally {
      setBusy(false)
    }
  }

  return {
    busy,
    error,
    disposition,
    available: (agentId: string) => target(agentId) !== undefined,
    interrupt: (agentId: string) => run(agentId),
    control: (agentId: string, command: AgentControlCommand) => run(agentId, command),
  }
}
