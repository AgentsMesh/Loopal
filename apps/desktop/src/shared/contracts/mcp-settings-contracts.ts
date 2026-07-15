import { z } from 'zod'

const IdentifierSchema = z.string().min(1).max(64).regex(/^[A-Za-z0-9][A-Za-z0-9_-]*$/)
const SafeTextSchema = (max: number, allowEmpty = false) => z.string().max(max).refine(
  (value) => (allowEmpty || value.trim().length > 0) && !/[\u0000-\u001f\u007f]/.test(value),
  'Invalid MCP text',
)
const TimeoutSchema = z.number().int().min(100).max(600_000)
export const McpSharingSchema = z.enum(['hub-singleton', 'per-agent', 'spawn-tree'])

export const McpSecretStatusSchema = z.object({
  name: z.string().min(1).max(128),
  configured: z.boolean(),
}).strict()
export type McpSecretStatus = z.infer<typeof McpSecretStatusSchema>

const SecretPatchBaseSchema = z.object({
  target: z.enum(['env', 'header']),
  name: z.string().min(1).max(128).refine((name) => !name.includes('.')),
})
const SetSecretPatchSchema = SecretPatchBaseSchema.extend({
  operation: z.literal('set'),
  value: z.string().min(1).max(8192).refine((value) => !value.includes('\0')),
}).strict()
const RemoveSecretPatchSchema = SecretPatchBaseSchema.extend({
  operation: z.literal('remove'),
}).strict()
export const McpSecretPatchSchema = z.discriminatedUnion('operation', [
  SetSecretPatchSchema, RemoveSecretPatchSchema,
])
export type McpSecretPatch = z.infer<typeof McpSecretPatchSchema>

export const McpCwdIsolationSchema = z.object({
  arg: SafeTextSchema(128).refine((value) => value.startsWith('-')),
  cacheSubdir: z.string().min(1).max(128).regex(/^[A-Za-z0-9][A-Za-z0-9_-]*$/).optional(),
}).strict()
export type McpCwdIsolation = z.infer<typeof McpCwdIsolationSchema>

const CommonDefinitionShape = {
  name: IdentifierSchema,
  source: z.enum(['global', 'plugin', 'project', 'local', 'environment', 'cli']),
  enabled: z.boolean(),
  timeoutMs: TimeoutSchema,
  sharing: McpSharingSchema,
}
export const StdioMcpServerDefinitionSchema = z.object({
  type: z.literal('stdio'),
  ...CommonDefinitionShape,
  command: SafeTextSchema(1024),
  args: z.array(SafeTextSchema(4096, true)).max(128),
  cwdIsolation: McpCwdIsolationSchema.nullable(),
  env: z.array(McpSecretStatusSchema),
}).strict()
export const HttpMcpServerDefinitionSchema = z.object({
  type: z.literal('streamable-http'),
  ...CommonDefinitionShape,
  url: SafeTextSchema(2048).refine(isPublicHttpUrl),
  headers: z.array(McpSecretStatusSchema),
}).strict()
export const McpServerDefinitionSchema = z.discriminatedUnion('type', [
  StdioMcpServerDefinitionSchema, HttpMcpServerDefinitionSchema,
])
export type McpServerDefinition = z.infer<typeof McpServerDefinitionSchema>

const CommonInputShape = {
  name: IdentifierSchema,
  enabled: z.boolean(),
  timeoutMs: TimeoutSchema,
  sharing: McpSharingSchema,
  secretPatches: z.array(McpSecretPatchSchema).max(256),
}
export const StdioMcpServerInputSchema = z.object({
  type: z.literal('stdio'),
  ...CommonInputShape,
  command: SafeTextSchema(1024),
  args: z.array(SafeTextSchema(4096, true)).max(128),
  cwdIsolation: McpCwdIsolationSchema.nullable(),
}).strict().superRefine((value, context) => {
  validateSecretTargets(value.secretPatches, 'env', context)
})
export const HttpMcpServerInputSchema = z.object({
  type: z.literal('streamable-http'),
  ...CommonInputShape,
  url: SafeTextSchema(2048).refine(isPublicHttpUrl),
}).strict().superRefine((value, context) => {
  validateSecretTargets(value.secretPatches, 'header', context)
  value.secretPatches.forEach((patch, index) => {
    if (patch.operation === 'set' && /[\u0000-\u001f\u007f]/.test(patch.value)) {
      context.addIssue({ code: 'custom', path: ['secretPatches', index], message: 'Invalid header' })
    }
  })
})
export const McpServerInputSchema = z.union([
  StdioMcpServerInputSchema, HttpMcpServerInputSchema,
])
export type McpServerInput = z.infer<typeof McpServerInputSchema>

export const McpServersResponseSchema = z.object({
  workspaceId: z.string().min(1),
  servers: z.array(McpServerDefinitionSchema),
}).strict()
export type McpServersResponse = z.infer<typeof McpServersResponseSchema>

export const ListMcpServersInputSchema = z.object({ workspaceId: z.string().min(1) }).strict()
export const UpsertMcpServerInputSchema = z.object({
  workspaceId: z.string().min(1), server: McpServerInputSchema,
}).strict()
export type UpsertMcpServerInput = z.infer<typeof UpsertMcpServerInputSchema>
export const DeleteMcpServerInputSchema = z.object({
  workspaceId: z.string().min(1), name: IdentifierSchema,
}).strict()
export type DeleteMcpServerInput = z.infer<typeof DeleteMcpServerInputSchema>

function isPublicHttpUrl(value: string): boolean {
  try {
    const url = new URL(value)
    return (url.protocol === 'http:' || url.protocol === 'https:')
      && !url.username && !url.password && !url.search && !url.hash
  } catch {
    return false
  }
}

function validateSecretTargets(
  patches: readonly McpSecretPatch[], target: McpSecretPatch['target'], context: z.RefinementCtx,
): void {
  const seen = new Set<string>()
  patches.forEach((patch, index) => {
    const identity = target === 'header' ? patch.name.toLowerCase() : patch.name
    if (patch.target !== target || seen.has(identity)) {
      context.addIssue({
        code: 'custom', path: ['secretPatches', index], message: `Invalid ${target} patch`,
      })
    }
    seen.add(identity)
  })
}
