import { type CancellationToken, throwIfCancelled } from '../../../../base/common/cancellation'
import {
  type PermissionResponseInput,
  type PlanApprovalResponseInput,
  type QuestionResponseInput,
} from '../../../../shared/contracts'
import { type CodeWorkbenchRuntimeRouter } from '../workspace/loopal-code-workbench'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

type Input = PermissionResponseInput | QuestionResponseInput | PlanApprovalResponseInput

export async function respondPermission(
  router: CodeWorkbenchRuntimeRouter, input: PermissionResponseInput, token: CancellationToken,
): Promise<void> {
  if (input.decision !== 'deny' && !input.intentDigest) {
    throw new Error('Permission intent digest is required to allow a tool')
  }
  await route(router, input, 'hub/permission_response', {
    agent_name: input.agentId, tool_call_id: input.requestId, allow: input.decision !== 'deny',
    ...(input.intentDigest ? { permission_intent_digest: input.intentDigest } : {}),
    ...(input.decision === 'allow_session' ? { remember_session: true } : {}),
  }, token)
}

export async function respondQuestion(
  router: CodeWorkbenchRuntimeRouter, input: QuestionResponseInput, token: CancellationToken,
): Promise<void> {
  await route(router, input, 'hub/question_response', {
    agent_name: input.agentId, question_id: input.requestId,
    response: input.cancelled ? { kind: 'cancelled', question_id: input.requestId }
      : { kind: 'answered', question_id: input.requestId, answers: input.answers ?? [] },
  }, token)
}

export async function respondPlanApproval(
  router: CodeWorkbenchRuntimeRouter, input: PlanApprovalResponseInput, token: CancellationToken,
): Promise<void> {
  await route(router, input, 'hub/plan_approval_response', {
    agent_name: input.agentId, request_id: input.requestId, decision: input.decision,
    ...(input.editedPlan !== undefined ? { edited_plan: input.editedPlan } : {}),
  }, token)
}

async function route(
  router: CodeWorkbenchRuntimeRouter,
  input: Input,
  method: string,
  params: unknown,
  token: CancellationToken,
): Promise<void> {
  throwIfCancelled(token)
  const runtime = await router.liveSession(input.sessionId)
  throwIfCancelled(token)
  assertRuntime(runtime, input)
  const controller = new AbortController()
  const subscription = token.onCancellationRequested(() => controller.abort())
  try {
    await runtime.host.request(method, params, controller.signal)
    throwIfCancelled(token)
  } finally {
    subscription.dispose()
  }
}

function assertRuntime(
  runtime: SessionRuntimeHandle | undefined,
  input: Input,
): asserts runtime is SessionRuntimeHandle {
  if (!runtime || runtime.runtimeId !== input.runtimeId || runtime.generation !== input.generation) {
    throw Object.assign(new Error(`Session runtime is gone: ${input.sessionId}`), {
      code: 'RUNTIME_GONE',
    })
  }
}
