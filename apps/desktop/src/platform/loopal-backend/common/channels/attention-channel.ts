import { type CancellationToken } from '../../../../base/common/cancellation'
import {
  PermissionResponseInputSchema,
  PlanApprovalResponseInputSchema,
  QuestionResponseInputSchema,
} from '../../../../shared/contracts'
import { type DesktopBackend } from '../backend'

export async function callAttentionBackend(
  backend: DesktopBackend,
  command: string,
  arg: unknown,
  token: CancellationToken,
): Promise<{ readonly handled: boolean; readonly value?: undefined }> {
  if (command === 'respondPermission') {
    await backend.respondPermission(PermissionResponseInputSchema.parse(arg), token)
  } else if (command === 'respondQuestion') {
    await backend.respondQuestion(QuestionResponseInputSchema.parse(arg), token)
  } else if (command === 'respondPlanApproval') {
    await backend.respondPlanApproval(PlanApprovalResponseInputSchema.parse(arg), token)
  } else return { handled: false }
  return { handled: true, value: undefined }
}
