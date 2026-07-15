import { type RuntimeEventNotice } from '../../../../shared/contracts'

export interface ProjectedEventNotice {
  readonly text: string
  readonly runtime?: RuntimeEventNotice
}

export function projectEventNotice(kind: string, value: unknown): ProjectedEventNotice | undefined {
  const fields = record(value)
  if (kind === 'SessionResumeWarnings') {
    const warnings = Array.isArray(fields?.warnings)
      ? fields.warnings.filter((item): item is string => typeof item === 'string')
      : []
    return warnings.length > 0 ? notice(`Session resume warning: ${warnings.join('; ')}`) : undefined
  }
  if (kind === 'Interrupted') return notice('Turn interrupted.')
  if (kind === 'TurnCancelled') return notice(detail('Turn cancelled', fields?.cause))
  if (kind === 'ContinuationSkipped') return notice(detail('Continuation skipped', fields?.reason))
  if (kind === 'DegenerationDetected') {
    const signal = label(fields?.signal)
    const count = number(fields?.count)
    return notice(`Degeneration detected${signal ? `: ${signal}` : ''}${count ? ` (${count})` : ''}.`)
  }
  if (kind === 'ContinuationGateChanged') {
    if (fields?.open === true) return notice('Automatic continuation resumed.')
    const reason = label(fields?.closed_reason)
    const deadline = typeof fields?.wake_deadline === 'string' ? fields.wake_deadline : undefined
    return notice(`Automatic continuation paused${reason ? `: ${reason}` : ''}${
      deadline ? ` until ${deadline}` : ''}.`)
  }
  const changed = changedNotice(kind, fields)
  if (changed) return changed
  if (kind === 'ThinkingChanged') {
    return runtimeNotice('Thinking configuration changed.', 'thinking_changed')
  }
  if (kind === 'Cleared') {
    return runtimeNotice('Conversation cleared.', 'conversation_cleared')
  }
  if (kind === 'Rewound') {
    const remaining = count(fields?.remaining_turns)
    return remaining === undefined
      ? notice('Conversation rewound.')
      : runtimeNotice(`Conversation rewound; ${remaining} turns remain.`, 'conversation_rewound', {
          remaining,
        })
  }
  if (kind === 'Compacted') {
    const before = count(fields?.tokens_before)
    const after = count(fields?.tokens_after)
    return before === undefined || after === undefined
      ? notice('Context compacted.')
      : runtimeNotice(`Context compacted: ${before} → ${after} tokens.`, 'context_compacted', {
          tokensBefore: before, tokensAfter: after,
        })
  }
  return undefined
}

function changedNotice(
  kind: string,
  fields: Record<string, unknown> | undefined,
): ProjectedEventNotice | undefined {
  const definitions = {
    ModeChanged: ['mode', 'Agent mode', 'mode_changed'],
    ModelChanged: ['model', 'Model', 'model_changed'],
    PermissionModeChanged: ['mode', 'Permission mode', 'permission_mode_changed'],
    DecisionModeChanged: ['mode', 'Decision mode', 'decision_mode_changed'],
    SandboxPolicyChanged: ['policy', 'Sandbox policy', 'sandbox_policy_changed'],
  } as const
  const definition = definitions[kind as keyof typeof definitions]
  if (!definition) return undefined
  const [field, subject, noticeKind] = definition
  const raw = string(fields?.[field])
  return raw
    ? runtimeNotice(`${subject} changed to ${label(raw)}.`, noticeKind, { value: raw })
    : undefined
}

function notice(text: string): ProjectedEventNotice {
  return { text }
}

function runtimeNotice(
  text: string,
  kind: RuntimeEventNotice['kind'],
  values?: RuntimeEventNotice['values'],
): ProjectedEventNotice {
  return { text, runtime: { kind, ...(values ? { values } : {}) } }
}

function detail(prefix: string, value: unknown): string {
  return typeof value === 'string' && value.length > 0 ? `${prefix}: ${value}` : `${prefix}.`
}

function label(value: unknown): string | undefined {
  const text = string(value)
  return text
    ? text.replaceAll('_', ' ')
    : undefined
}

function number(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) && value > 0 ? value : undefined
}

function count(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : undefined
}

function string(value: unknown): string | undefined {
  return typeof value === 'string' && value.length > 0 ? value : undefined
}

function record(value: unknown): Record<string, unknown> | undefined {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined
}
