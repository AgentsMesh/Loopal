import { type ChannelClient } from '../../../ipc/common/channel'
import {
  type PermissionResponseInput,
  type PlanApprovalResponseInput,
  type QuestionResponseInput,
} from '../../../../shared/contracts'

export interface AttentionClientOperations {
  respondPermission(input: PermissionResponseInput): Promise<void>
  respondQuestion(input: QuestionResponseInput): Promise<void>
  respondPlanApproval(input: PlanApprovalResponseInput): Promise<void>
}

export function bindAttentionClient(client: ChannelClient): AttentionClientOperations {
  const call = async (command: string, input: unknown): Promise<void> => {
    await client.call('desktopBackend', command, input)
  }
  return {
    respondPermission: (input) => call('respondPermission', input),
    respondQuestion: (input) => call('respondQuestion', input),
    respondPlanApproval: (input) => call('respondPlanApproval', input),
  }
}
