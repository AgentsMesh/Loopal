import { type RuntimeEventNotice } from '../../../../shared/contracts'
import { type MessageKey } from '../../../../shared/i18n'

type Values = Readonly<Record<string, string | number>>
type Translate = (key: MessageKey, values?: Values) => string

const NOTICE_KEYS: Readonly<Record<RuntimeEventNotice['kind'], MessageKey>> = {
  mode_changed: 'runtimeNotice.modeChanged',
  model_changed: 'runtimeNotice.modelChanged',
  thinking_changed: 'runtimeNotice.thinkingChanged',
  permission_mode_changed: 'runtimeNotice.permissionChanged',
  decision_mode_changed: 'runtimeNotice.decisionChanged',
  sandbox_policy_changed: 'runtimeNotice.sandboxChanged',
  conversation_cleared: 'runtimeNotice.cleared',
  conversation_rewound: 'runtimeNotice.rewound',
  context_compacted: 'runtimeNotice.compacted',
}

export function localizeRuntimeEventNotice(
  notice: RuntimeEventNotice,
  t: Translate,
): string {
  const values = { ...notice.values }
  if (typeof values.value === 'string') {
    values.value = localizeValue(notice.kind, values.value, t)
  }
  return t(NOTICE_KEYS[notice.kind], values)
}

function localizeValue(kind: RuntimeEventNotice['kind'], value: string, t: Translate): string {
  if (kind === 'model_changed') return value
  const normalized = value.toLowerCase()
  const key = valueKey(kind, normalized)
  return key ? t(key) : value.replaceAll('_', ' ')
}

function valueKey(
  kind: RuntimeEventNotice['kind'],
  value: string,
): MessageKey | undefined {
  if (kind === 'mode_changed') {
    return value === 'act' ? 'composer.act' : value === 'plan' ? 'composer.plan' : undefined
  }
  if (kind === 'permission_mode_changed') return permissionKey(value)
  if (kind === 'decision_mode_changed') return decisionKey(value)
  if (kind === 'sandbox_policy_changed') return sandboxKey(value)
  return undefined
}

function permissionKey(value: string): MessageKey | undefined {
  if (value === 'bypass') return 'settings.loopal.permission.bypass'
  if (value === 'ask_dangerous') return 'settings.loopal.permission.dangerous'
  if (value === 'ask_any_write') return 'settings.loopal.permission.write'
  return undefined
}

function decisionKey(value: string): MessageKey | undefined {
  if (value === 'manual') return 'settings.loopal.decision.manual'
  if (value === 'classifier') return 'settings.loopal.decision.classifier'
  if (value === 'agent') return 'settings.loopal.decision.agent'
  return undefined
}

function sandboxKey(value: string): MessageKey | undefined {
  if (value === 'default_write') return 'settings.loopal.sandbox.write'
  if (value === 'read_only') return 'settings.loopal.sandbox.readOnly'
  if (value === 'disabled') return 'settings.loopal.disabled'
  return undefined
}
