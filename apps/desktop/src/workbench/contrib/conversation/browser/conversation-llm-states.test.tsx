import { render, screen, within } from '@testing-library/react'
import {
  type ConversationEntry, type SessionDetail, type SessionView, type ToolInvocation,
} from '../../../../shared/contracts'
import { ConversationView } from './conversation-view'
import { SessionRuntimeStatus } from '../../sessions/browser/session-runtime-status'

const createdAt = '2026-07-12T12:00:00.000Z'

describe('authoritative LLM state DOM', () => {
  it('renders provider banners, continuations, server tools, failures, and notices', () => {
    render(<ConversationView entries={entries()} view={view()} />)

    expect(screen.getByText('Earlier history is not loaded in this view.')).toBeVisible()
    expect(screen.getByText('Provider unavailable. Retrying in 2.0s (1/6)')).toBeVisible()
    expect(screen.getByText('⠉ summarizing context — model request')).toBeVisible()
    expect(screen.getByText(/Output truncated \(max_tokens\)/)).toBeVisible()
    expect(screen.getByText(/Context compacted \(smart\)/)).toBeVisible()
    expect(screen.getByText('Degeneration detected: barren streak (3).').closest('article'))
      .toHaveClass('event-notice')
    expect(screen.getByText(/Automatic continuation paused/).closest('article'))
      .toHaveClass('event-notice')

    const server = row('web_search')
    expect(within(server).getByLabelText('Completed')).toBeVisible()
    expect(server).toHaveTextContent('two verified sources')
    const failed = row('Write')
    expect(failed).toHaveAttribute('open')
    expect(within(failed).getByLabelText('Failed')).toBeVisible()
    expect(failed).toHaveTextContent('permission denied')
    expect(screen.getByText('provider request failed fatally').closest('article'))
      .toHaveAttribute('data-message-role', 'error')
    expect(screen.getByLabelText('Streaming')).toBeVisible()
  })

  it('gives compaction and fatal failure the correct runtime status priority', () => {
    const { rerender } = render(<SessionRuntimeStatus detail={detail('running', view())} />)
    expect(status()).toHaveTextContent('Compacting')
    rerender(<SessionRuntimeStatus detail={detail('failed', view({
      compactBanner: null, streamingText: '', retryBanner: null,
    }))} />)
    expect(status()).toHaveTextContent('Failed')
  })
})

function entries(): ConversationEntry[] {
  return [
    entry('system', 'Output truncated (max_tokens). Auto-continuing (1/8)'),
    entry('system', 'Context compacted (smart): 8→2 messages, 75% freed.'),
    { ...entry('system', 'Degeneration detected: barren streak (3).'), eventNotice: true },
    { ...entry('system', 'Automatic continuation paused: idle timeout.'), eventNotice: true },
    { ...entry('assistant', 'Tool results'), toolCalls: [
      tool('search', 'web_search', 'succeeded', { output: 'two verified sources' }),
      tool('write', 'Write', 'failed', { detail: 'permission denied' }),
    ] },
    { ...entry('assistant', 'partial response before continuation'), streaming: true },
    entry('error', 'provider request failed fatally'),
  ]
}

function entry(role: ConversationEntry['role'], text: string): ConversationEntry {
  return { id: `${role}-${text}`, role, text, createdAt }
}

function tool(
  id: string, name: string, status: ToolInvocation['status'],
  fields: Partial<ToolInvocation>,
): ToolInvocation {
  return { id, name, summary: `${name}(fixture)`, status, ...fields }
}

function view(patch: Partial<SessionView> = {}): SessionView {
  return {
    revision: 9, historyTruncated: true,
    streamingText: 'partial response before continuation', streamingThinking: '',
    thinkingActive: false, retryBanner: 'Provider unavailable. Retrying in 2.0s (1/6)',
    compactBanner: '⠉ summarizing context — model request',
    tasks: [], backgroundTasks: [], crons: [], mcpServers: [], ...patch,
  }
}

function detail(status: 'running' | 'failed', state: SessionView): SessionDetail {
  return {
    session: {
      id: 'session', workspaceId: 'workspace', title: 'LLM states', model: 'model',
      mode: 'act', status, createdAt, updatedAt: createdAt,
      ...(status === 'running' ? { activeRuntimeId: 'runtime' } : {}),
    },
    conversation: [], artifacts: [], view: state,
    agents: [{ id: 'main', name: 'Loopal', status, model: 'model', mode: 'act' }],
  }
}

function row(name: string) {
  return screen.getAllByTestId('tool-invocation').find((element) => (
    element.textContent?.includes(name)
  ))!
}

function status(): HTMLElement {
  return screen.getByTestId('runtime-status').querySelector('strong')!
}
