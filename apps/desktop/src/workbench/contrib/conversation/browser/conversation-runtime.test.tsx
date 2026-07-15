import { fireEvent, render, screen } from '@testing-library/react'
import { type AgentSummary, type ConversationEntry, type ToolInvocation } from '../../../../shared/contracts'
import {
  richAgent, richDetail, richTimestamp, richView,
} from '../../../../../test/fixtures/workbench/rich-session'
import { ConversationView } from './conversation-view'
import { SessionRuntimeStatus } from '../../sessions/browser/session-runtime-status'
import { ToolInvocationView } from './tool-invocation-view'
describe('ConversationView', () => {
  it('renders roles, live banners, attachments, skills, inbox, tools, and streaming', () => {
    const entries: ConversationEntry[] = [{
      id: 'assistant', role: 'assistant', text: '**Ready**', createdAt: richTimestamp,
      agentId: 'main', imageCount: 2, streaming: true,
      inbox: { source: 'worker', summary: 'result' },
      toolCalls: [tool('running', { progress: 'halfway' })],
    }, {
      id: 'thinking', role: 'thinking', text: '', createdAt: richTimestamp,
      agentId: 'worker', imageCount: 0, toolCalls: [], thinkingTokens: 42,
    }, {
      id: 'user', role: 'user', text: 'internal expanded prompt', createdAt: richTimestamp,
      skill: { name: 'desktop', userArgs: 'verify' },
    }, {
      id: 'system', role: 'system', text: 'Recovered', createdAt: richTimestamp,
      eventNotice: true,
    }, {
      id: 'error', role: 'error', text: 'Failed', createdAt: richTimestamp,
    }, {
      id: 'welcome', role: 'welcome', text: 'Welcome', createdAt: richTimestamp,
    }]
    const view = richView({
      historyTruncated: true, retryBanner: 'Retrying', compactBanner: 'Compacted',
    })
    const { rerender } = render(<ConversationView entries={entries} view={view} />)

    expect(screen.getByText('Earlier history is not loaded in this view.')).toBeInTheDocument()
    expect(screen.getByText('Retrying')).toBeInTheDocument()
    expect(screen.getByText('Compacted')).toBeInTheDocument()
    expect(screen.getByText('Thinking')).toBeInTheDocument()
    expect(screen.getByText('verify').closest('article')).toHaveAccessibleName('User')
    expect(screen.getByText('Recovered').closest('article')).toHaveAccessibleName('System')
    expect(screen.getByText('Recovered').closest('article')).toHaveClass('event-notice')
    expect(screen.getByText('Error')).toBeInTheDocument()
    expect(screen.getAllByRole('article', { name: 'Loopal' })).toHaveLength(2)
    expect(screen.getByText('42 tokens')).toBeInTheDocument()
    expect(screen.getByText('worker')).toBeInTheDocument()
    expect(screen.getByText('Skill · desktop')).toBeInTheDocument()
    expect(screen.getByText('verify')).toBeInTheDocument()
    expect(screen.queryByText('internal expanded prompt')).not.toBeInTheDocument()
    expect(screen.getByText('From · worker')).toBeInTheDocument()
    expect(screen.getByText('2 image attachment(s)')).toBeInTheDocument()
    expect(screen.getByLabelText('Streaming')).toBeInTheDocument()
    expect(screen.getByTestId('tool-invocation')).toBeInTheDocument()

    rerender(<ConversationView entries={[]} />)
    expect(screen.queryByText('Retrying')).not.toBeInTheDocument()
  })
  it('follows live output until the reader scrolls away from the tail', () => {
    const entry = (id: string): ConversationEntry => ({
      id, role: 'assistant', text: id, createdAt: richTimestamp,
    })
    const { rerender } = render(
      <div data-testid="scroll"><ConversationView entries={[entry('one')]} /></div>,
    )
    const scroll = screen.getByTestId('scroll')
    Object.defineProperty(scroll, 'scrollHeight', { configurable: true, value: 500 })
    Object.defineProperty(scroll, 'clientHeight', { configurable: true, value: 100 })
    scroll.scrollTop = 400
    fireEvent.scroll(scroll)
    rerender(<div data-testid="scroll"><ConversationView entries={[entry('one'), entry('two')]} /></div>)
    expect(scroll.scrollTop).toBe(500)

    scroll.scrollTop = 0
    fireEvent.scroll(scroll)
    rerender(<div data-testid="scroll"><ConversationView entries={[entry('three')]} /></div>)
    expect(scroll.scrollTop).toBe(0)
  })
})

describe('ToolInvocationView', () => {
  it('renders every state, duration, payload shape, and fallback safely', () => {
    const cyclic: Record<string, unknown> = {}
    cyclic.self = cyclic
    const tools: ToolInvocation[] = [
      tool('pending', { summary: 'Bash({"cmd":"bazel test //..."})' }),
      tool('running', {
        summary: '', input: 'bazel test //...', progress: 'running',
        detail: 'Command active', output: 'PASS', durationMs: 250,
      }),
      tool('succeeded', { input: { target: '//...' }, output: 'done', durationMs: 1_250 }),
      tool('failed', { input: cyclic, detail: 'exit 1' }),
      tool('stale'),
      tool('cancelled'),
    ]
    render(<>{tools.map((item) => <ToolInvocationView tool={item} key={item.status} />)}</>)

    for (const label of ['Queued', 'Running', 'Completed', 'Failed', 'Stale', 'Cancelled']) {
      expect(screen.getByLabelText(label)).toBeInTheDocument()
    }
    const rows = screen.getAllByTestId('tool-invocation')
    expect(rows[0]).toHaveAttribute('open')
    expect(rows[0]!.querySelector('strong')).toHaveTextContent(/^Bash$/)
    expect(rows[0]).not.toHaveTextContent('"cmd"')
    expect(rows[1]).toHaveAttribute('open')
    expect(rows[2]).not.toHaveAttribute('open')
    expect(rows[3]).toHaveAttribute('open')
    expect(screen.getByText('250ms', { exact: false })).toBeInTheDocument()
    expect(screen.getByText('1.3s', { exact: false })).toBeInTheDocument()
    expect(screen.getByText('bazel test //...')).toBeInTheDocument()
    expect(screen.getByText(/"target": "\/\/\.\.\."/)).toBeInTheDocument()
    expect(screen.getByText('[object Object]')).toBeInTheDocument()
  })
})

describe('SessionRuntimeStatus', () => {
  it('projects thinking, work, every lifecycle state, context, and fallbacks', () => {
    const { rerender } = render(<SessionRuntimeStatus detail={richDetail({
      agents: [richAgent()], view: richView({ thinkingActive: true }),
    })} />)
    const status = () => screen.getByTestId('runtime-status').querySelector('strong')!
    expect(status()).toHaveTextContent('Thinking')
    expect(screen.getByTestId('runtime-status')).toHaveTextContent('Context 20%')
    const working = richAgent({
      lastTool: 'Running tests', telemetry: {
        ...richAgent().telemetry!, turnCount: 2, inputTokens: 2_000,
        contextWindow: 1_000, toolsInFlight: 2,
      },
    })
    rerender(<SessionRuntimeStatus detail={detailWith(working)} />)
    expect(status()).toHaveTextContent('Working')
    expect(screen.getByTestId('runtime-status')).toHaveTextContent('Context 100%')
    for (const [role, label] of [
      ['thinking', 'Thinking'], ['assistant', 'Streaming'],
    ] as const) {
      rerender(<SessionRuntimeStatus detail={detailWith(richAgent({ conversation: [{
        id: role, role, text: 'live', streaming: true, createdAt: richTimestamp,
      }] }))} />)
      expect(status()).toHaveTextContent(label)
    }
    const views = [[{ compactBanner: 'Compacting context' }, 'Compacting'],
      [{ streamingText: 'live text' }, 'Streaming']] as const
    for (const [patch, label] of views) {
      rerender(<SessionRuntimeStatus detail={richDetail({
        agents: [richAgent({ view: undefined })],
        view: richView({ thinkingActive: false, compactBanner: null, ...patch }),
      })} />)
      expect(status()).toHaveTextContent(label)
    }
    rerender(<SessionRuntimeStatus detail={richDetail({
      agents: [richAgent()], session: { ...richDetail().session, attention: 'question' },
      view: richView({ thinkingActive: false, compactBanner: null }),
    })} />)
    expect(status()).toHaveTextContent('Waiting for answer')
    rerender(<SessionRuntimeStatus detail={richDetail({
      agents: [richAgent(), richAgent({ id: 'one', parentId: 'main' })],
      view: richView({ thinkingActive: false, compactBanner: null }),
    })} />)
    expect(status()).toHaveTextContent('1 Agent working')
    for (const [agentStatus, label] of [
      ['waiting', 'Ready for input'], ['starting', 'Starting'], ['suspended', 'Suspended'],
      ['completed', 'Completed'], ['failed', 'Failed'], ['idle', 'Idle'],
    ] as const) {
      rerender(<SessionRuntimeStatus detail={detailWith(richAgent({ status: agentStatus }))} />)
      expect(status()).toHaveTextContent(label)
    }
    rerender(<SessionRuntimeStatus detail={detailWith({
      id: 'worker', name: 'Worker', status: 'running', model: 'mini', mode: 'plan',
    })} />)
    expect(status()).toHaveTextContent('Working')
    rerender(<SessionRuntimeStatus detail={detailWith(richAgent({
      id: 'worker', name: 'Worker', parentId: 'main',
      view: richView({ thinkingActive: true }),
    }))} />)
    expect(status()).toHaveTextContent('Thinking')
    rerender(<SessionRuntimeStatus detail={detailWith({
      id: 'worker', name: 'Worker', parentId: 'main', status: 'running',
    })} agentId="worker" />)
    expect(status()).toHaveTextContent('Working')
    rerender(<SessionRuntimeStatus detail={richDetail({
      agents: [], view: richView({ thinkingActive: false }),
    })} />)
    expect(status()).toHaveTextContent('running')
    expect(screen.getByTestId('runtime-status')).not.toHaveTextContent('Context')
  })
})
function tool(
  status: ToolInvocation['status'],
  overrides: Partial<ToolInvocation> = {},
): ToolInvocation {
  return { id: `tool-${status}`, name: 'Bash', summary: `Tool ${status}`, status, ...overrides }
}

function detailWith(agent: AgentSummary) {
  const detail = richDetail({
    agents: [agent],
    view: richView({
      thinkingActive: false, streamingThinking: '', streamingText: '', compactBanner: null,
    }),
  })
  return detail
}
