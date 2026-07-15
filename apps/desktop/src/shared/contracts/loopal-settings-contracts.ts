import { z } from 'zod'

const SafeTextSchema = (max: number, allowEmpty = false) => z.string().max(max).refine(
  (value) => (allowEmpty || value.trim().length > 0) && !/[\u0000-\u001f\u007f]/.test(value),
  'Invalid settings text',
)
const PublicBaseUrlSchema = SafeTextSchema(2048, true).refine(
  isSafePublicBaseUrl, 'Base URL must be a public http(s) URL without credentials, query, or fragment',
)

export const LoopalThinkingSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('auto') }).strict(),
  z.object({ type: z.literal('disabled') }).strict(),
  z.object({
    type: z.literal('effort'),
    level: z.enum(['low', 'medium', 'high', 'max']),
  }).strict(),
  z.object({
    type: z.literal('budget'),
    tokens: z.number().int().min(1).max(4_294_967_295),
  }).strict(),
])
export type LoopalThinking = z.infer<typeof LoopalThinkingSchema>

export const LoopalModelRoutingSchema = z.object({
  default: SafeTextSchema(256, true),
  summarization: SafeTextSchema(256, true),
  classification: SafeTextSchema(256, true),
  refine: SafeTextSchema(256, true),
}).strict()
export type LoopalModelRouting = z.infer<typeof LoopalModelRoutingSchema>

export const LoopalSettingsValuesSchema = z.object({
  model: SafeTextSchema(256),
  modelRouting: LoopalModelRoutingSchema,
  permissionMode: z.enum(['bypass', 'ask_dangerous', 'ask_any_write']),
  decisionMode: z.enum(['manual', 'classifier', 'agent']),
  sandboxPolicy: z.enum(['disabled', 'default_write', 'read_only']),
  thinking: LoopalThinkingSchema,
  maxContextTokens: z.number().int().min(0).max(4_294_967_295),
  memoryEnabled: z.boolean(),
  microcompactIdleMinutes: z.number().int().min(0).max(1440),
  telemetryEnabled: z.boolean(),
  outputStyle: SafeTextSchema(128, true),
}).strict()
export type LoopalSettingsValues = z.infer<typeof LoopalSettingsValuesSchema>

export const LoopalProviderSettingsSchema = z.object({
  enabled: z.boolean(),
  baseUrl: PublicBaseUrlSchema,
  apiKeyEnv: SafeTextSchema(128, true),
  apiKeyConfigured: z.boolean(),
}).strict()
export type LoopalProviderSettings = z.infer<typeof LoopalProviderSettingsSchema>

export const LoopalBuiltInProvidersSchema = z.object({
  anthropic: LoopalProviderSettingsSchema,
  openai: LoopalProviderSettingsSchema,
  google: LoopalProviderSettingsSchema,
}).strict()
export type LoopalBuiltInProviders = z.infer<typeof LoopalBuiltInProvidersSchema>

export const LoopalOpenAiCompatibleSettingsSchema = z.object({
  name: SafeTextSchema(96),
  baseUrl: PublicBaseUrlSchema,
  apiKeyEnv: SafeTextSchema(128, true),
  modelPrefix: SafeTextSchema(128, true),
  apiKeyConfigured: z.boolean(),
}).strict()
export type LoopalOpenAiCompatibleSettings = z.infer<
  typeof LoopalOpenAiCompatibleSettingsSchema
>

export const LoopalResolvedSettingEntrySchema = z.object({
  key: z.string().min(1).max(512),
  value: z.string().max(512),
}).strict()

export const LoopalDefaultSettingsSchema = z.object({
  workspaceId: z.string().min(1),
  settings: LoopalSettingsValuesSchema,
  configuredProviders: z.array(z.string().max(128)).max(64),
  providers: LoopalBuiltInProvidersSchema,
  openaiCompatible: z.array(LoopalOpenAiCompatibleSettingsSchema).max(61),
  resolvedEntries: z.array(LoopalResolvedSettingEntrySchema).max(1024),
  settingSources: z.array(z.string().min(1).max(128)).max(32),
}).strict()
export type LoopalDefaultSettings = z.infer<typeof LoopalDefaultSettingsSchema>

export const LoopalSettingsWorkspaceInputSchema = z.object({
  workspaceId: z.string().min(1),
}).strict()
export type LoopalSettingsWorkspaceInput = z.infer<typeof LoopalSettingsWorkspaceInputSchema>

export const LoopalProviderUpdateSchema = z.object({
  enabled: z.boolean().optional(),
  remove: z.boolean().optional(),
  baseUrl: PublicBaseUrlSchema.optional(),
  apiKeyEnv: SafeTextSchema(128, true).regex(/^$|^[A-Za-z_][A-Za-z0-9_]*$/).optional(),
  apiKey: SafeTextSchema(8192).optional(),
  clearApiKey: z.boolean().optional(),
}).strict().superRefine((value, context) => {
  const changes = value.baseUrl !== undefined || value.apiKeyEnv !== undefined
    || value.apiKey !== undefined || value.clearApiKey === true
  if (value.remove && (value.enabled !== undefined || changes)) {
    context.addIssue({ code: 'custom', message: 'Remove cannot be combined with other changes' })
  }
  if (value.enabled === false && changes) {
    context.addIssue({ code: 'custom', message: 'Disable cannot be combined with field changes' })
  }
  if (value.apiKey !== undefined && value.clearApiKey) {
    context.addIssue({ code: 'custom', message: 'API key cannot be set and cleared together' })
  }
})
export type LoopalProviderUpdate = z.infer<typeof LoopalProviderUpdateSchema>

export const LoopalOpenAiCompatibleUpdateSchema = z.object({
  name: SafeTextSchema(96),
  remove: z.boolean().optional(),
  baseUrl: PublicBaseUrlSchema.optional(),
  apiKeyEnv: SafeTextSchema(128, true).regex(/^$|^[A-Za-z_][A-Za-z0-9_]*$/).optional(),
  modelPrefix: SafeTextSchema(128, true).optional(),
  apiKey: SafeTextSchema(8192).optional(),
  clearApiKey: z.boolean().optional(),
}).strict().superRefine((value, context) => {
  const changes = value.baseUrl !== undefined || value.apiKeyEnv !== undefined
    || value.modelPrefix !== undefined || value.apiKey !== undefined || value.clearApiKey === true
  if (value.remove && changes) {
    context.addIssue({ code: 'custom', message: 'Remove cannot include field changes' })
  }
  if (value.apiKey !== undefined && value.clearApiKey) {
    context.addIssue({ code: 'custom', message: 'API key cannot be set and cleared together' })
  }
})
export type LoopalOpenAiCompatibleUpdate = z.infer<
  typeof LoopalOpenAiCompatibleUpdateSchema
>

export const LoopalProviderUpdatesSchema = z.object({
  anthropic: LoopalProviderUpdateSchema.optional(),
  openai: LoopalProviderUpdateSchema.optional(),
  google: LoopalProviderUpdateSchema.optional(),
  openaiCompatible: z.array(LoopalOpenAiCompatibleUpdateSchema).max(61).optional(),
}).strict()
export type LoopalProviderUpdates = z.infer<typeof LoopalProviderUpdatesSchema>

export const UpdateLoopalSettingsInputSchema = z.object({
  workspaceId: z.string().min(1),
  settings: LoopalSettingsValuesSchema,
  providerUpdates: LoopalProviderUpdatesSchema.optional(),
}).strict()
export type UpdateLoopalSettingsInput = z.infer<typeof UpdateLoopalSettingsInputSchema>

function isSafePublicBaseUrl(value: string): boolean {
  if (value === '') return true
  try {
    const url = new URL(value)
    return (url.protocol === 'http:' || url.protocol === 'https:')
      && url.username === '' && url.password === '' && url.search === '' && url.hash === ''
      && !value.includes('@') && !value.includes('?') && !value.includes('#')
  } catch {
    return false
  }
}
