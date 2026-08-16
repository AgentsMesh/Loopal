import {
  type Stage2WorkbenchCallbacks,
  type Stage2WorkbenchModel,
} from '../../../src/workbench/browser/stage2-view-model'

export const stage2Model: Stage2WorkbenchModel = {
  context: {
    workspaces: [
      { id: 'workspace', name: 'Loopal', detail: '/work/loopal' },
      { id: 'docs', name: 'Docs', detail: '/work/docs' },
    ],
    activeWorkspaceId: 'workspace',
    sessions: [
      { id: 'session-1', workspaceId: 'workspace', title: 'Desktop', state: 'running' },
      { id: 'session-2', workspaceId: 'workspace', title: 'Protocol', state: 'waiting' },
      { id: 'docs-1', workspaceId: 'docs', title: 'Guide', state: 'idle' },
    ],
    activeSessionId: 'session-1',
  },
  permissions: [
    { id: 'write', agentId: 'main', title: 'Write files', description: 'Modify the workspace.', risk: 'medium', canAllow: true, command: 'apply_patch' },
    { id: 'network', title: 'Use network', description: 'Contact the registry.', risk: 'high', canAllow: false },
  ],
  questions: [{
    id: 'style',
    agentId: 'worker',
    prompt: 'Which interface should be used?',
    allowMultiple: false,
    selectedChoiceIds: [],
    otherText: '',
    choices: [
      { id: 'compact', label: 'Compact', description: 'Use less space.' },
      { id: 'comfortable', label: 'Comfortable' },
    ],
  }],
  planApprovals: [],
}

export function createStage2Callbacks(): Stage2WorkbenchCallbacks {
  return {
    onWorkspaceChange: vi.fn(),
    onSessionChange: vi.fn(),
    onResolvePermission: vi.fn(),
    onAnswerQuestion: vi.fn(),
    onQuestionFreeTextChange: vi.fn(),
    onSubmitQuestionAnswers: vi.fn(),
    onCancelQuestion: vi.fn(),
    onPlanApprovalEdit: vi.fn(),
    onResolvePlanApproval: vi.fn(),
  }
}
