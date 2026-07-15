import { z } from 'zod'

export const DesktopLocalePreferenceSchema = z.enum(['system', 'en', 'zh-CN'])
export type DesktopLocalePreference = z.infer<typeof DesktopLocalePreferenceSchema>

export const DesktopPreferencesSchema = z.object({
  locale: DesktopLocalePreferenceSchema,
}).strict()
export type DesktopPreferences = z.infer<typeof DesktopPreferencesSchema>

export const UpdateDesktopPreferencesInputSchema = DesktopPreferencesSchema
export type UpdateDesktopPreferencesInput = z.infer<
  typeof UpdateDesktopPreferencesInputSchema
>

export const DEFAULT_DESKTOP_PREFERENCES: DesktopPreferences = { locale: 'system' }
