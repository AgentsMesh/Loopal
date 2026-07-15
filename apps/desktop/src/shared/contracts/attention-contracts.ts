import { z } from 'zod'

export const PermissionRequestSchema = z.object({
  id: z.string().min(1),
  sessionId: z.string().min(1),
  runtimeId: z.string().min(1),
  generation: z.number().int().positive(),
  agentId: z.string().min(1),
  tool: z.string().min(1),
  title: z.string().min(1),
  detail: z.string(),
  risk: z.enum(['low', 'medium', 'high']),
  createdAt: z.string().datetime(),
})
export type PermissionRequest = z.infer<typeof PermissionRequestSchema>

export const QuestionItemSchema = z.object({
  question: z.string().min(1),
  header: z.string().optional(),
  options: z.array(z.object({ label: z.string().min(1), description: z.string() })),
  allowMultiple: z.boolean(),
})
export const QuestionClassifierStatusSchema = z.object({
  kind: z.enum(['none', 'running', 'failed', 'completed']),
  elapsedMs: z.number().int().nonnegative().optional(),
  reason: z.string().optional(),
  answers: z.array(z.string()).optional(),
})
export const QuestionRequestSchema = z.object({
  id: z.string().min(1),
  sessionId: z.string().min(1),
  runtimeId: z.string().min(1),
  generation: z.number().int().positive(),
  agentId: z.string().min(1),
  questions: z.array(QuestionItemSchema).min(1).max(20),
  classifierRunning: z.boolean(),
  classifierStatus: QuestionClassifierStatusSchema.optional(),
  createdAt: z.string().datetime(),
})
export type QuestionRequest = z.infer<typeof QuestionRequestSchema>

export const PlanApprovalRequestSchema = z.object({
  id: z.string().min(1),
  sessionId: z.string().min(1),
  runtimeId: z.string().min(1),
  generation: z.number().int().positive(),
  agentId: z.string().min(1),
  planContent: z.string().min(1).max(1_000_000),
  planPath: z.string().min(1).max(10_000),
  createdAt: z.string().datetime(),
})
export type PlanApprovalRequest = z.infer<typeof PlanApprovalRequestSchema>

export const PermissionResponseInputSchema = z.object({
  sessionId: z.string().min(1),
  runtimeId: z.string().min(1),
  generation: z.number().int().positive(),
  agentId: z.string().min(1),
  requestId: z.string().min(1),
  decision: z.enum(['allow_once', 'allow_session', 'deny']),
})
export const QuestionResponseInputSchema = z.object({
  sessionId: z.string().min(1),
  runtimeId: z.string().min(1),
  generation: z.number().int().positive(),
  agentId: z.string().min(1),
  requestId: z.string().min(1),
  answers: z.array(z.string().max(100_000)).min(1).max(20).optional(),
  cancelled: z.literal(true).optional(),
}).superRefine((input, context) => {
  if (Boolean(input.cancelled) === Boolean(input.answers?.length)) {
    context.addIssue({
      code: z.ZodIssueCode.custom,
      message: 'Provide answers or cancel the question, but not both',
    })
  }
})
export type PermissionResponseInput = z.infer<typeof PermissionResponseInputSchema>
export type QuestionResponseInput = z.infer<typeof QuestionResponseInputSchema>
export const PlanApprovalResponseInputSchema = z.object({
  sessionId: z.string().min(1), runtimeId: z.string().min(1),
  generation: z.number().int().positive(), agentId: z.string().min(1),
  requestId: z.string().min(1),
  decision: z.enum(['approve', 'reject', 'approve_with_edits']),
  editedPlan: z.string().max(1_000_000).optional(),
}).superRefine((input, context) => {
  if (input.decision === 'approve_with_edits' && input.editedPlan === undefined) {
    context.addIssue({ code: z.ZodIssueCode.custom, message: 'Edited plan is required' })
  }
})
export type PlanApprovalResponseInput = z.infer<typeof PlanApprovalResponseInputSchema>

export const AttentionDesktopEventSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('permission_requested'), request: PermissionRequestSchema }),
  z.object({
    type: z.literal('permission_resolved'),
    sessionId: z.string().min(1),
    runtimeId: z.string().min(1),
    generation: z.number().int().positive(),
    agentId: z.string().min(1),
    requestId: z.string().min(1),
  }),
  z.object({ type: z.literal('question_requested'), request: QuestionRequestSchema }),
  z.object({
    type: z.literal('question_resolved'),
    sessionId: z.string().min(1),
    runtimeId: z.string().min(1),
    generation: z.number().int().positive(),
    agentId: z.string().min(1),
    requestId: z.string().min(1),
  }),
  z.object({ type: z.literal('plan_approval_requested'), request: PlanApprovalRequestSchema }),
  z.object({
    type: z.literal('plan_approval_resolved'),
    sessionId: z.string().min(1), runtimeId: z.string().min(1),
    generation: z.number().int().positive(), agentId: z.string().min(1),
    requestId: z.string().min(1),
  }),
])
