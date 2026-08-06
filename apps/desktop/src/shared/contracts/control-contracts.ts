import { z } from 'zod'

const IdentifierSchema = z.string().trim().min(1).max(512)
const ValueSchema = z.string().trim().min(1).max(4_096)

export const AgentControlTargetSchema = z.object({
  sessionId: IdentifierSchema,
  runtimeId: IdentifierSchema,
  generation: z.number().int().positive(),
  agentId: IdentifierSchema,
}).strict()
export type AgentControlTarget = z.infer<typeof AgentControlTargetSchema>

export const ThinkingConfigSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('auto') }).strict(),
  z.object({ type: z.literal('disabled') }).strict(),
  z.object({
    type: z.literal('effort'),
    level: z.enum(['none', 'low', 'medium', 'high', 'xhigh', 'max']),
  }).strict(),
  z.object({
    type: z.literal('budget'),
    tokens: z.number().int().positive().max(4_294_967_295),
  }).strict(),
])
export type ThinkingConfig = z.infer<typeof ThinkingConfigSchema>

export const AgentControlCommandSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('mode'), mode: z.enum(['act', 'plan']) }).strict(),
  z.object({ type: z.literal('clear') }).strict(),
  z.object({
    type: z.literal('compact'),
    instructions: ValueSchema.optional(),
  }).strict(),
  z.object({ type: z.literal('model'), model: ValueSchema }).strict(),
  z.object({
    type: z.literal('rewind'),
    turnIndex: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER),
  }).strict(),
  z.object({ type: z.literal('thinking'), config: ThinkingConfigSchema }).strict(),
  z.object({
    type: z.literal('permission'),
    mode: z.enum(['bypass', 'ask_dangerous', 'ask_any_write']),
  }).strict(),
  z.object({
    type: z.literal('decision'),
    mode: z.enum(['manual', 'classifier', 'agent']),
  }).strict(),
  z.object({
    type: z.literal('sandbox'),
    policy: z.enum(['disabled', 'default_write', 'read_only']),
  }).strict(),
  z.object({ type: z.literal('suspend') }).strict(),
  z.object({ type: z.literal('unsuspend') }).strict(),
  z.object({ type: z.literal('mcp_status') }).strict(),
  z.object({ type: z.literal('mcp_reconnect'), server: IdentifierSchema }).strict(),
  z.object({ type: z.literal('mcp_disconnect'), server: IdentifierSchema }).strict(),
  z.object({ type: z.literal('background_task_kill'), id: IdentifierSchema }).strict(),
  z.object({ type: z.literal('cron_delete'), id: IdentifierSchema }).strict(),
])
export type AgentControlCommand = z.infer<typeof AgentControlCommandSchema>

export const AgentControlInputSchema = z.object({
  target: AgentControlTargetSchema,
  command: AgentControlCommandSchema,
}).strict()
export type AgentControlInput = z.infer<typeof AgentControlInputSchema>
