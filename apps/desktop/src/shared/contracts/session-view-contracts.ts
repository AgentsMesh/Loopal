import { z } from 'zod'

export const ToolInvocationSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  summary: z.string(),
  status: z.enum(['pending', 'running', 'succeeded', 'failed', 'stale', 'cancelled']),
  input: z.unknown().optional(),
  output: z.string().optional(),
  progress: z.string().optional(),
  detail: z.string().optional(),
  durationMs: z.number().nonnegative().optional(),
  batchId: z.string().optional(),
})
export type ToolInvocation = z.infer<typeof ToolInvocationSchema>

export const AgentTelemetrySchema = z.object({
  turnCount: z.number().int().nonnegative(),
  inputTokens: z.number().int().nonnegative(),
  outputTokens: z.number().int().nonnegative(),
  cacheCreationTokens: z.number().int().nonnegative(),
  cacheReadTokens: z.number().int().nonnegative(),
  thinkingTokens: z.number().int().nonnegative(),
  contextWindow: z.number().int().nonnegative(),
  toolsInFlight: z.number().int().nonnegative(),
  toolCount: z.number().int().nonnegative(),
})
export type AgentTelemetry = z.infer<typeof AgentTelemetrySchema>

export const TaskSummarySchema = z.object({
  id: z.string().min(1),
  subject: z.string().min(1),
  description: z.string(),
  activeForm: z.string().optional(),
  status: z.enum(['pending', 'in_progress', 'completed']),
  blockedBy: z.array(z.string()),
  blocks: z.array(z.string()),
})
export type TaskSummary = z.infer<typeof TaskSummarySchema>

export const BackgroundTaskSchema = z.object({
  id: z.string().min(1),
  description: z.string(),
  status: z.enum(['running', 'completed', 'failed', 'killed']),
  exitCode: z.number().int().nullable(),
  output: z.string(),
  createdAt: z.string().datetime(),
})
export type BackgroundTask = z.infer<typeof BackgroundTaskSchema>

export const CronJobSchema = z.object({
  id: z.string().min(1),
  schedule: z.string(),
  prompt: z.string(),
  recurring: z.boolean(),
  durable: z.boolean(),
  nextFireAt: z.string().datetime().optional(),
})
export type CronJob = z.infer<typeof CronJobSchema>

export const McpServerSchema = z.object({
  name: z.string().min(1),
  transport: z.string(),
  source: z.string(),
  status: z.string(),
  toolCount: z.number().int().nonnegative(),
  resourceCount: z.number().int().nonnegative(),
  promptCount: z.number().int().nonnegative(),
  errors: z.array(z.string()),
})
export type McpServer = z.infer<typeof McpServerSchema>

export const ThreadGoalSchema = z.object({
  id: z.string().min(1),
  objective: z.string().min(1),
  status: z.enum(['active', 'paused', 'complete', 'infeasible']),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
})
export type ThreadGoal = z.infer<typeof ThreadGoalSchema>

export const WorkflowStateCountsSchema = z.object({
  pending: z.number().int().nonnegative(),
  ready: z.number().int().nonnegative(),
  active: z.number().int().nonnegative(),
  succeeded: z.number().int().nonnegative(),
  failed: z.number().int().nonnegative(),
  cancelled: z.number().int().nonnegative(),
  skipped: z.number().int().nonnegative(),
})
export type WorkflowStateCounts = z.infer<typeof WorkflowStateCountsSchema>

export const WorkflowRunSummarySchema = z.object({
  id: z.string().min(1),
  runGoal: z.string(),
  state: z.enum([
    'planned', 'validated', 'running', 'cancelling', 'succeeded', 'failed', 'cancelled',
  ]),
  revision: z.number().int().nonnegative(),
  outputNode: z.string().min(1),
  counts: WorkflowStateCountsSchema,
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
})
export type WorkflowRunSummary = z.infer<typeof WorkflowRunSummarySchema>

export const WorkflowRunsSchema = z.object({
  active: z.array(WorkflowRunSummarySchema),
  recent: z.array(WorkflowRunSummarySchema),
})
export type WorkflowRuns = z.infer<typeof WorkflowRunsSchema>

export const SessionViewSchema = z.object({
  revision: z.number().int().nonnegative(),
  historyTruncated: z.boolean(),
  streamingText: z.string(),
  streamingThinking: z.string(),
  thinkingActive: z.boolean(),
  retryBanner: z.string().nullable(),
  compactBanner: z.string().nullable(),
  tasks: z.array(TaskSummarySchema),
  backgroundTasks: z.array(BackgroundTaskSchema),
  crons: z.array(CronJobSchema),
  mcpServers: z.array(McpServerSchema),
  workflows: WorkflowRunsSchema.default({ active: [], recent: [] }),
  goal: ThreadGoalSchema.optional(),
  hubDegradedSince: z.string().datetime().optional(),
})
export type SessionView = z.infer<typeof SessionViewSchema>
