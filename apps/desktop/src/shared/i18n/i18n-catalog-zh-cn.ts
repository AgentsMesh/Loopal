import { type MessageKey } from './i18n-catalog-en'
import { ACTIVITY_ZH_CN } from './messages-activity-zh-cn'
import { COMMON_ZH_CN } from './messages-common-zh-cn'
import { SHELL_ZH_CN } from './messages-shell-zh-cn'
import { SETTINGS_CORE_ZH_CN } from './messages-settings-core-zh-cn'
import { SETTINGS_LOOPAL_ZH_CN } from './messages-settings-loopal-zh-cn'
import { SETTINGS_MCP_ZH_CN } from './messages-settings-mcp-zh-cn'
import { SETTINGS_SKILLS_ZH_CN } from './messages-settings-skills-zh-cn'
import { SETTINGS_METAHUB_ZH_CN } from './messages-settings-metahub-zh-cn'
import { PANELS_ZH_CN } from './messages-panels-zh-cn'
import { CODE_ZH_CN } from './messages-code-zh-cn'
import { FEDERATION_ZH_CN } from './messages-federation-zh-cn'
import { SESSION_CREATE_ZH_CN } from './messages-session-create-zh-cn'

export const ZH_CN_MESSAGES = {
  ...COMMON_ZH_CN,
  ...ACTIVITY_ZH_CN,
  ...SHELL_ZH_CN,
  ...SETTINGS_CORE_ZH_CN,
  ...SETTINGS_LOOPAL_ZH_CN,
  ...SETTINGS_MCP_ZH_CN,
  ...SETTINGS_SKILLS_ZH_CN,
  ...SETTINGS_METAHUB_ZH_CN,
  ...PANELS_ZH_CN,
  ...CODE_ZH_CN,
  ...FEDERATION_ZH_CN,
  ...SESSION_CREATE_ZH_CN,
} as const satisfies Readonly<Record<MessageKey, string>>
