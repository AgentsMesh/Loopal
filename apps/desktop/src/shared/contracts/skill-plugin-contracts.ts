import { z } from 'zod'

const WorkspaceIdSchema = z.string().min(1).max(256)
const ManagedSkillNameSchema = z.string()
  .regex(/^\/[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/, 'Invalid canonical skill name')
const SkillDisplayNameSchema = z.string().min(2).max(256).refine(
  (value) => value.startsWith('/') && !/[\\/\u0000-\u001f\u007f]/.test(value.slice(1)),
  'Invalid skill display name',
)
const DescriptionSchema = z.string().max(512).refine(
  (value) => !/[\u0000-\u001f\u007f]/.test(value),
  'Invalid skill description',
)
const ManagedDescriptionSchema = DescriptionSchema.refine(
  (value) => value.trim().length > 0, 'Skill description is required',
)
const SkillBodySchema = z.string().min(1).max(100 * 1024).refine(
  (value) => !value.includes('\0'), 'Skill body must not contain NUL',
)
const LegacySkillBodySchema = z.string().max(100 * 1024)
const RevisionSchema = z.string().regex(/^[a-f0-9]{64}$/, 'Invalid skill revision')
const SourceSchema = z.string().min(1).max(256).refine(
  (value) => !/[\u0000-\u001f\u007f]/.test(value), 'Invalid configuration source',
)

export const SkillScopeSchema = z.enum(['global', 'project', 'plugin'])

export const SkillSummarySchema = z.object({
  name: SkillDisplayNameSchema,
  description: DescriptionSchema,
  hasArguments: z.boolean(),
  source: SourceSchema,
  scope: SkillScopeSchema,
  editable: z.boolean(),
  effective: z.boolean(),
  revision: RevisionSchema.optional(),
}).strict()
export type SkillSummary = z.infer<typeof SkillSummarySchema>

export const SkillsResponseSchema = z.object({
  workspaceId: WorkspaceIdSchema,
  skills: z.array(SkillSummarySchema).max(2048),
}).strict()
export type SkillsResponse = z.infer<typeof SkillsResponseSchema>

export const SkillDetailSchema = SkillSummarySchema.extend({
  name: ManagedSkillNameSchema,
  workspaceId: WorkspaceIdSchema,
  body: LegacySkillBodySchema,
  revision: RevisionSchema,
}).strict()
export type SkillDetail = z.infer<typeof SkillDetailSchema>

export const ListSkillPluginsInputSchema = z.object({
  workspaceId: WorkspaceIdSchema,
}).strict()

export const GetSkillInputSchema = z.object({
  workspaceId: WorkspaceIdSchema,
  name: ManagedSkillNameSchema,
}).strict()
export type GetSkillInput = z.infer<typeof GetSkillInputSchema>

export const UpsertGlobalSkillInputSchema = z.object({
  workspaceId: WorkspaceIdSchema,
  name: ManagedSkillNameSchema,
  description: ManagedDescriptionSchema,
  body: SkillBodySchema,
  expectedRevision: RevisionSchema.optional(),
}).strict().superRefine((value, context) => {
  const serialized = `---\ndescription: ${value.description}\n---\n${value.body}`
  if (new TextEncoder().encode(serialized).byteLength > 100 * 1024) {
    context.addIssue({
      code: 'custom', path: ['body'], message: 'Serialized skill exceeds 100 KiB',
    })
  }
})
export type UpsertGlobalSkillInput = z.infer<typeof UpsertGlobalSkillInputSchema>

export const DeleteGlobalSkillInputSchema = z.object({
  workspaceId: WorkspaceIdSchema,
  name: ManagedSkillNameSchema,
  expectedRevision: RevisionSchema,
}).strict()
export type DeleteGlobalSkillInput = z.infer<typeof DeleteGlobalSkillInputSchema>

export const PluginSummarySchema = z.object({
  name: z.string().min(1).max(128).refine(
    (value) => ![/[\\/]/, /[\u0000-\u001f\u007f]/].some((pattern) => pattern.test(value)),
    'Invalid Plugin name',
  ),
  source: SourceSchema,
  skills: z.array(SkillDisplayNameSchema).max(1024),
  mcpServers: z.array(z.string().min(1).max(128)).max(1024),
  hookCount: z.number().int().min(0).max(65_535),
  hasSettings: z.boolean(),
  hasInstructions: z.boolean(),
  hasMemory: z.boolean(),
}).strict()
export type PluginSummary = z.infer<typeof PluginSummarySchema>

export const PluginsResponseSchema = z.object({
  workspaceId: WorkspaceIdSchema,
  plugins: z.array(PluginSummarySchema).max(1024),
}).strict()
export type PluginsResponse = z.infer<typeof PluginsResponseSchema>
