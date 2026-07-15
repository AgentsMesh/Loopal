import { ACTIVITY_EN } from './messages-activity-en'
import { COMMON_EN } from './messages-common-en'
import { SHELL_EN } from './messages-shell-en'
import { SETTINGS_CORE_EN } from './messages-settings-core-en'
import { SETTINGS_LOOPAL_EN } from './messages-settings-loopal-en'
import { SETTINGS_MCP_EN } from './messages-settings-mcp-en'
import { SETTINGS_SKILLS_EN } from './messages-settings-skills-en'
import { SETTINGS_METAHUB_EN } from './messages-settings-metahub-en'
import { PANELS_EN } from './messages-panels-en'
import { CODE_EN } from './messages-code-en'
import { FEDERATION_EN } from './messages-federation-en'
import { SESSION_CREATE_EN } from './messages-session-create-en'

export const EN_MESSAGES = {
  ...COMMON_EN,
  ...ACTIVITY_EN,
  ...SHELL_EN,
  ...SETTINGS_CORE_EN,
  ...SETTINGS_LOOPAL_EN,
  ...SETTINGS_MCP_EN,
  ...SETTINGS_SKILLS_EN,
  ...SETTINGS_METAHUB_EN,
  ...PANELS_EN,
  ...CODE_EN,
  ...FEDERATION_EN,
  ...SESSION_CREATE_EN,
} as const

export type MessageKey = keyof typeof EN_MESSAGES
