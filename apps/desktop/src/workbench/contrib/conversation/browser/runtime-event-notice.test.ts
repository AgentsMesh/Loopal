import {
  ConversationEntrySchema,
  type RuntimeEventNotice,
} from '../../../../shared/contracts'
import { translate, type MessageKey, type SupportedLocale } from '../../../../shared/i18n'
import { localizeRuntimeEventNotice } from './runtime-event-notice'

describe('runtime event notice localization', () => {
  it('localizes runtime configuration values in English and Chinese', () => {
    expect(localize('en', notice('mode_changed', { value: 'plan' })))
      .toBe('Agent mode changed to Plan.')
    expect(localize('zh-CN', notice('mode_changed', { value: 'plan' })))
      .toBe('Agent 模式已切换为“规划”。')
    expect(localize('zh-CN', notice('permission_mode_changed', {
      value: 'ask_dangerous',
    }))).toBe('权限模式已切换为“危险操作时询问”。')
    expect(localize('zh-CN', notice('decision_mode_changed', { value: 'manual' })))
      .toBe('决策模式已切换为“手动”。')
    expect(localize('zh-CN', notice('sandbox_policy_changed', { value: 'read_only' })))
      .toBe('沙箱策略已切换为“只读”。')
    expect(localize('en', notice('model_changed', { value: 'custom_model' })))
      .toBe('Model changed to custom_model.')
  })

  it('localizes authoritative conversation outcomes and validates the wire contract', () => {
    expect(localize('zh-CN', notice('conversation_cleared'))).toBe('对话已清除。')
    expect(localize('en', notice('conversation_rewound', { remaining: 2 })))
      .toBe('Conversation rewound; 2 turns remain.')
    const compacted = notice('context_compacted', {
      tokensBefore: 8_000, tokensAfter: 3_000,
    })
    expect(localize('zh-CN', compacted)).toBe('上下文已压缩：8000 → 3000 token。')
    expect(ConversationEntrySchema.safeParse({
      id: 'event-1', role: 'system', text: 'Context compacted.',
      createdAt: '2026-07-14T12:00:00.000Z', eventNotice: compacted,
    }).success).toBe(true)
  })
})

function notice(
  kind: RuntimeEventNotice['kind'],
  values?: RuntimeEventNotice['values'],
): RuntimeEventNotice {
  return { kind, ...(values ? { values } : {}) }
}

function localize(locale: SupportedLocale, runtime: RuntimeEventNotice): string {
  return localizeRuntimeEventNotice(runtime, (
    key: MessageKey, values?: Readonly<Record<string, string | number>>,
  ) => translate(locale, key, values))
}
