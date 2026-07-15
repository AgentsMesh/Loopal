import { z } from 'zod'
import {
  AgentTelemetrySchema,
  SessionViewSchema,
  ToolInvocationSchema,
} from './session-view-contracts'
import { MetaHubRuntimeStateSchema } from './metahub-contracts'
import {
  DesktopImageAttachmentListSchema,
} from './image-contracts'

export * from './session-view-contracts'

export const SessionStatusSchema = z.enum([
  'stopped',
  'starting',
  'running',
  'waiting',
  'failed',
  'archived',
])
export type SessionStatus = z.infer<typeof SessionStatusSchema>

export const SessionSummarySchema = z.object({
  id: z.string().min(1),
  workspaceId: z.string().min(1),
  title: z.string().min(1),
  model: z.string().min(1),
  mode: z.string().min(1),
  status: SessionStatusSchema,
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
  activeRuntimeId: z.string().min(1).optional(),
  attention: z.enum(['permission', 'question', 'plan', 'failure', 'completed']).optional(),
})
export type SessionSummary = z.infer<typeof SessionSummarySchema>

export const RuntimeSummarySchema = z.object({
  id: z.string().min(1),
  sessionId: z.string().min(1),
  workspaceId: z.string().min(1),
  generation: z.number().int().positive(),
  state: z.enum(['starting', 'ready', 'stopping', 'stopped', 'crashed']),
  rootAgent: z.string().min(1),
  startedAt: z.string().datetime().optional(),
})
export type RuntimeSummary = z.infer<typeof RuntimeSummarySchema>

export const RuntimeEventNoticeKindSchema = z.enum([
  'mode_changed',
  'model_changed',
  'thinking_changed',
  'permission_mode_changed',
  'decision_mode_changed',
  'sandbox_policy_changed',
  'conversation_cleared',
  'conversation_rewound',
  'context_compacted',
])
export const RuntimeEventNoticeSchema = z.object({
  kind: RuntimeEventNoticeKindSchema,
  values: z.record(z.string(), z.union([z.string(), z.number()])).optional(),
}).strict()
export type RuntimeEventNotice = z.infer<typeof RuntimeEventNoticeSchema>

export const ConversationEntrySchema = z.object({
  id: z.string().min(1),
  role: z.enum(['user', 'assistant', 'system', 'thinking', 'error', 'welcome']),
  text: z.string(),
  createdAt: z.string().datetime(),
  agentId: z.string().optional(),
  imageCount: z.number().int().nonnegative().optional(),
  toolCalls: z.array(ToolInvocationSchema).optional(),
  skill: z.object({ name: z.string(), userArgs: z.string() }).optional(),
  inbox: z.object({ source: z.string(), summary: z.string().optional() }).optional(),
  streaming: z.boolean().optional(),
  thinkingTokens: z.number().int().nonnegative().optional(),
  eventNotice: z.union([z.boolean(), RuntimeEventNoticeSchema]).optional(),
})
export type ConversationEntry = z.infer<typeof ConversationEntrySchema>

export const AgentSummarySchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  status: z.enum([
    'starting', 'idle', 'running', 'waiting', 'suspended', 'completed', 'failed',
  ]),
  parentId: z.string().optional(),
  children: z.array(z.string()).optional(),
  model: z.string().optional(),
  mode: z.string().optional(),
  thinkingConfig: z.string().optional(),
  permissionMode: z.string().optional(),
  decisionMode: z.string().optional(),
  sandboxPolicy: z.string().optional(),
  lastTool: z.string().optional(),
  telemetry: AgentTelemetrySchema.optional(),
  conversation: z.array(ConversationEntrySchema).optional(),
  view: SessionViewSchema.optional(),
  controllable: z.boolean().optional(),
  hubPath: z.array(z.string().min(1)).optional(),
  qualifiedName: z.string().min(1).optional(),
  shadow: z.boolean().optional(),
  error: z.string().optional(),
})
export type AgentSummary = z.infer<typeof AgentSummarySchema>

export const ArtifactSchema = z.object({
  id: z.string().min(1),
  sessionId: z.string().min(1),
  title: z.string().min(1),
  kind: z.enum(['document', 'code', 'image', 'report', 'web_app', 'other']),
  uri: z.string().min(1),
  mediaType: z.string().min(1),
  producerAgentId: z.string().min(1),
  createdAt: z.string().datetime(),
})
export type Artifact = z.infer<typeof ArtifactSchema>

export const SessionDetailSchema = z.object({
  session: SessionSummarySchema,
  conversation: z.array(ConversationEntrySchema),
  agents: z.array(AgentSummarySchema),
  artifacts: z.array(ArtifactSchema),
  view: SessionViewSchema.optional(),
  metaHub: MetaHubRuntimeStateSchema.optional(),
})
export type SessionDetail = z.infer<typeof SessionDetailSchema>

export const SessionOperationInputSchema = z.object({ sessionId: z.string().min(1) })
export const SessionDirectorySelectionSchema = z.object({
  authorizationId: z.string().uuid(),
  path: z.string().min(1),
  name: z.string().min(1),
  git: z.object({
    root: z.string().min(1), branch: z.string().min(1).optional(), dirty: z.boolean(),
  }).strict().optional(),
  suggestedWorktreeName: z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/u),
}).strict()
export type SessionDirectorySelection = z.infer<typeof SessionDirectorySelectionSchema>

const WorktreeNameSchema = z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/u)
export const CreateSessionInputSchema = z.discriminatedUnion('launchMode', [
  z.object({
    authorizationId: z.string().uuid(),
    launchMode: z.literal('directory'),
    worktreeName: z.never().optional(),
  }).strict(),
  z.object({
    authorizationId: z.string().uuid(),
    launchMode: z.literal('worktree'),
    worktreeName: WorktreeNameSchema,
  }).strict(),
])
export const SendMessageInputSchema = z.object({
  sessionId: z.string().min(1),
  text: z.string().trim().max(100_000),
  agentId: z.string().min(1).max(512).optional(),
  images: DesktopImageAttachmentListSchema.optional(),
}).refine((input) => input.text.length > 0 || Boolean(input.images?.length), {
  message: 'A message needs text or an image',
})

export type CreateSessionInput = z.infer<typeof CreateSessionInputSchema>
