import { z } from 'zod'

export const MetaHubSettingsSchema = z.object({
  address: z.string().max(512),
  hubName: z.string().min(1).max(128),
  joinOnStart: z.boolean(),
  startLocalOnLaunch: z.boolean(),
  tokenConfigured: z.boolean(),
})
export type MetaHubSettings = z.infer<typeof MetaHubSettingsSchema>

export const UpdateMetaHubSettingsInputSchema = z.object({
  address: z.string().trim().max(512),
  hubName: z.string().trim().min(1).max(128).refine((value) => !value.includes('/'), {
    message: "Hub name cannot contain '/'",
  }),
  joinOnStart: z.boolean(),
  startLocalOnLaunch: z.boolean(),
  token: z.string().max(4096).optional(),
  clearToken: z.boolean().optional(),
})
export type UpdateMetaHubSettingsInput = z.infer<typeof UpdateMetaHubSettingsInputSchema>

export const MetaHubRuntimeTargetSchema = z.object({
  sessionId: z.string().min(1),
  runtimeId: z.string().min(1),
  generation: z.number().int().positive(),
})
export type MetaHubRuntimeTarget = z.infer<typeof MetaHubRuntimeTargetSchema>

export const MetaHubInfoSchema = z.object({
  name: z.string().min(1),
  status: z.enum(['connected', 'degraded', 'disconnected']),
  agentCount: z.number().int().nonnegative(),
  capabilities: z.array(z.string()),
})
export type MetaHubInfo = z.infer<typeof MetaHubInfoSchema>

export const MetaHubTopologyAgentSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  hub: z.string().min(1),
  hubPath: z.array(z.string().min(1)).min(1),
  parentId: z.string().optional(),
  children: z.array(z.string()),
  lifecycle: z.enum(['spawning', 'running', 'finished', 'failed']),
  model: z.string().optional(),
  error: z.string().optional(),
})
export type MetaHubTopologyAgent = z.infer<typeof MetaHubTopologyAgentSchema>

export const MetaHubRuntimeStateSchema = z.object({
  state: z.enum(['disconnected', 'connected', 'error']),
  address: z.string().optional(),
  hubName: z.string().optional(),
  hubs: z.array(MetaHubInfoSchema),
  topology: z.array(MetaHubTopologyAgentSchema),
  error: z.string().optional(),
  refreshedAt: z.string().datetime(),
})
export type MetaHubRuntimeState = z.infer<typeof MetaHubRuntimeStateSchema>

export const JoinMetaHubInputSchema = MetaHubRuntimeTargetSchema.extend({
  address: z.string().trim().min(1).max(512).optional(),
  hubName: z.string().trim().min(1).max(128).optional(),
  token: z.string().min(1).max(4096).optional(),
})
export type JoinMetaHubInput = z.infer<typeof JoinMetaHubInputSchema>

export const LocalMetaHubStatusSchema = z.object({
  state: z.enum(['stopped', 'starting', 'running', 'failed']),
  address: z.string().optional(),
  error: z.string().optional(),
})
export type LocalMetaHubStatus = z.infer<typeof LocalMetaHubStatusSchema>

export const StartLocalMetaHubInputSchema = z.object({
  bindAddress: z.string().trim().min(1).max(512).default('127.0.0.1:0'),
})
export type StartLocalMetaHubInput = z.infer<typeof StartLocalMetaHubInputSchema>
