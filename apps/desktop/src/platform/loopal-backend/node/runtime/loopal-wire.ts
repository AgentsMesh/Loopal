import { z } from 'zod'

const ToolStateSchema = z.object({ state: z.string() }).passthrough()
const WireToolSchema = z.object({
  id: z.string(),
  name: z.string(),
  summary: z.string().default(''),
  input: z.unknown().optional(),
  state: ToolStateSchema,
  batch_id: z.string().nullable().optional(),
  metadata: z.unknown().nullable().optional(),
})

const WireMessageSchema = z.object({
  role: z.string(),
  content: z.string(),
  tool_calls: z.array(WireToolSchema).default([]),
  image_count: z.number().int().nonnegative().default(0),
  skill_info: z.object({ name: z.string(), user_args: z.string() }).nullable().optional(),
  inbox: z.object({
    message_id: z.string(),
    source: z.unknown(),
    summary: z.string().nullable().optional(),
  }).nullable().optional(),
  message_id: z.string().optional(),
  ui_local: z.boolean().default(false),
})

const WireConversationSchema = z.object({
  messages: z.array(WireMessageSchema).default([]),
  history_truncated: z.boolean().default(false),
  streaming_text: z.string().default(''),
  streaming_thinking: z.string().default(''),
  thinking_active: z.boolean().default(false),
  pending_permission: z.object({
    id: z.string(), name: z.string(), input: z.unknown(), cursor: z.string().optional(),
  }).nullable().optional(),
  pending_question: z.object({
    id: z.string(),
    questions: z.array(z.object({
      question: z.string(), header: z.string().nullable().optional(),
      options: z.array(z.object({ label: z.string(), description: z.string() })),
      allow_multiple: z.boolean(),
    })),
    classifier_status: z.object({ kind: z.string() }).passthrough().optional(),
  }).nullable().optional(),
  pending_plan_approval: z.object({
    id: z.string(), plan_content: z.string(), plan_path: z.string(),
  }).nullable().optional(),
  retry_banner: z.string().nullable().optional(),
  compact_banner: z.string().nullable().optional(),
  turn_count: z.number().int().nonnegative().default(0),
  input_tokens: z.number().int().nonnegative().default(0),
  output_tokens: z.number().int().nonnegative().default(0),
  context_window: z.number().int().nonnegative().default(0),
  cache_creation_tokens: z.number().int().nonnegative().default(0),
  cache_read_tokens: z.number().int().nonnegative().default(0),
  thinking_tokens: z.number().int().nonnegative().default(0),
})

const WireAgentSchema = z.object({
  name: z.string(),
  session_id: z.string().nullable().optional(),
  parent: z.string().nullable().optional(),
  children: z.array(z.string()).default([]),
  observable: z.object({
    status: z.string(),
    turn_count: z.number().int().nonnegative().default(0),
    input_tokens: z.number().int().nonnegative().default(0),
    output_tokens: z.number().int().nonnegative().default(0),
    model: z.string().default(''),
    thinking_config: z.string().default('auto'),
    mode: z.string().default('act'),
    permission_mode: z.string().default(''),
    decision_mode: z.string().default(''),
    sandbox_policy: z.string().default(''),
  }),
  conversation: WireConversationSchema,
})

const WireTaskSchema = z.object({
  id: z.string(), subject: z.string(), description: z.string().default(''),
  active_form: z.string().nullable().optional(), status: z.string(),
  blocked_by: z.array(z.string()).default([]), blocks: z.array(z.string()).default([]),
})
const WireBackgroundTaskSchema = z.object({
  id: z.string(), description: z.string(), status: z.string(),
  exit_code: z.number().int().nullable().optional(), output: z.string().default(''),
  created_at_unix_ms: z.number().nonnegative(),
})
const WireCronSchema = z.object({
  id: z.string(), cron_expr: z.string().default(''), prompt: z.string(),
  recurring: z.boolean(), durable: z.boolean().default(false),
  next_fire_unix_ms: z.number().nullable().optional(),
})
const WireMcpSchema = z.object({
  name: z.string(), transport: z.string(), source: z.string(), status: z.string(),
  tool_count: z.number().int().nonnegative(), resource_count: z.number().int().nonnegative(),
  prompt_count: z.number().int().nonnegative(), errors: z.array(z.string()).default([]),
})
const WireGoalSchema = z.object({
  goal_id: z.string(), objective: z.string(), status: z.string(),
  created_at: z.string(), updated_at: z.string(),
})

export const ViewSnapshotSchema = z.object({
  rev: z.number().int().nonnegative(),
  state: z.object({
    agent: WireAgentSchema,
    tasks: z.array(WireTaskSchema).default([]),
    crons: z.array(WireCronSchema).default([]),
    bg_tasks: z.record(z.string(), WireBackgroundTaskSchema).default({}),
    mcp_status: z.array(WireMcpSchema).nullable().optional(),
    thread_goal: WireGoalSchema.nullable().optional(),
    hub_degraded_since_ms: z.number().nonnegative().nullable().optional(),
  }),
})
export type ViewSnapshot = z.infer<typeof ViewSnapshotSchema>

export const AgentListSchema = z.object({
  agents: z.array(z.object({ name: z.string(), state: z.string() })),
})

export const TopologySchema = z.object({
  agents: z.array(z.object({
    name: z.string(), parent: z.string().nullable().optional(),
    children: z.array(z.string()).default([]),
    lifecycle: z.enum(['spawning', 'running', 'finished', 'failed']),
    error: z.string().nullable().optional(),
    model: z.string().nullable().optional(),
    shadow: z.boolean().optional(),
  })),
})
export type Topology = z.infer<typeof TopologySchema>

export const AgentEventSchema = z.object({
  agent_name: z.object({ hub: z.array(z.string()), agent: z.string() }).nullable().optional(),
  event_id: z.number().optional(),
  rev: z.number().int().nonnegative().optional(),
  payload: z.unknown(),
})
