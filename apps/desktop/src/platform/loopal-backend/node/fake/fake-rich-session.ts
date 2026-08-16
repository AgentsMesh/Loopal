import {
  type AgentSummary,
  type ConversationEntry,
  type SessionView,
} from '../../../../shared/contracts'

export function fakeRichConversation(now: string): ConversationEntry[] {
  return [{
    id: 'desktop-assistant', role: 'assistant', agentId: 'agent-root', createdAt: now,
    text: '# Verified desktop state\n\n**Bazel-only** bridge is ready.\n\n- Renderer connected\n- Host ready\n\n```ts\nconst desktop = ready\n```',
    imageCount: 1,
    skill: { name: 'desktop', userArgs: 'verify' },
    toolCalls: [{
      id: 'tool-build', name: 'Bash', summary: 'Build LoopalDesktop', status: 'succeeded',
      input: { command: 'bazel build //apps/desktop:out' }, output: 'Build completed',
      durationMs: 1_250,
    }],
  }, {
    id: 'desktop-thinking', role: 'thinking', agentId: 'agent-root', createdAt: now,
    text: 'Checking the renderer and Host state.', streaming: true,
  }, {
    id: 'desktop-resume-warning', role: 'system', agentId: 'agent-root', createdAt: now,
    text: 'Session resume warning: one scheduled job needs review.', eventNotice: true,
  }]
}

export function fakeRichAgents(): AgentSummary[] {
  return [{
    id: 'agent-root', name: 'Loopal', status: 'running', model: 'gpt-5', mode: 'act',
    thinkingConfig: 'auto', permissionMode: 'ask_dangerous', decisionMode: 'classifier',
    sandboxPolicy: 'default_write', lastTool: 'Running Electron verification',
    children: ['agent-e2e'],
    telemetry: {
      turnCount: 3, inputTokens: 1_200, outputTokens: 320,
      cacheCreationTokens: 40, cacheReadTokens: 80, thinkingTokens: 90,
      contextWindow: 200_000, toolsInFlight: 1, toolCount: 4,
    },
  }, {
    id: 'agent-e2e', name: 'E2E specialist', status: 'waiting', parentId: 'agent-root',
    model: 'gpt-5-mini', mode: 'act',
    conversation: [{
      id: 'agent-e2e-result', role: 'assistant', agentId: 'agent-e2e',
      text: 'E2E specialist verified the Electron renderer.',
      createdAt: '2026-07-11T12:00:00.000Z',
    }],
  }]
}

export function fakeSessionView(now: string): SessionView {
  return {
    revision: 7, historyTruncated: true,
    streamingText: '', streamingThinking: 'Checking state', thinkingActive: true,
    retryBanner: 'Retrying one transient provider response.',
    compactBanner: 'Summarizing context for the next turn.',
    goal: {
      id: 'goal-desktop', objective: 'Ship a verified Loopal Desktop', status: 'active',
      createdAt: now, updatedAt: now,
    },
    tasks: [{
      id: 'task-shell', subject: 'Wire the desktop shell', description: 'Connect every pane.',
      activeForm: 'Wiring the desktop shell', status: 'in_progress', blockedBy: [],
      blocks: ['task-e2e'],
    }, {
      id: 'task-e2e', subject: 'Verify the Electron workbench', description: 'Run Playwright.',
      status: 'pending', blockedBy: ['task-shell'], blocks: [],
    }],
    backgroundTasks: [{
      id: 'bg-bazel', description: 'Bazel test runner', status: 'running', exitCode: null,
      output: 'Testing //apps/desktop:e2e', createdAt: now,
    }],
    crons: [{
      id: 'cron-health', schedule: '*/15 * * * *', prompt: 'Check Desktop Host health',
      recurring: true, durable: true, nextFireAt: now,
    }],
    mcpServers: [{
      name: 'filesystem', transport: 'stdio', source: 'builtin', status: 'ready',
      toolCount: 8, resourceCount: 2, promptCount: 1, errors: [],
    }],
    workflows: { active: [], recent: [] },
  }
}
