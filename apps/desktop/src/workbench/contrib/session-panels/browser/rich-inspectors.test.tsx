import { fireEvent, render, screen } from '@testing-library/react'
import { type SessionView } from '../../../../shared/contracts'
import {
  richAgent, richDetail, richTimestamp, richView,
} from '../../../../../test/fixtures/workbench/rich-session'
import { DiagnosticsInspector } from './diagnostics-inspector'
import { TaskInspector } from './task-inspector'

describe('TaskInspector', () => {
  it('renders goals, plans, dependencies, background work, and schedules', () => {
    const view = richView({
      tasks: [{
        id: 'done', subject: 'Finished task', description: 'Verified output',
        activeForm: 'Finishing task', status: 'completed', blockedBy: ['setup'],
        blocks: ['ship'],
      }, {
        id: 'next', subject: 'Pending task', description: '', status: 'pending',
        blockedBy: [], blocks: [],
      }],
      backgroundTasks: [{
        id: 'complete', description: 'Completed command', status: 'completed',
        exitCode: 0, output: 'PASS', createdAt: richTimestamp,
      }, {
        id: 'running', description: 'Live command', status: 'running',
        exitCode: null, output: '', createdAt: richTimestamp,
      }],
      crons: [{
        id: 'scheduled', schedule: '0 * * * *', prompt: 'Hourly check',
        recurring: true, durable: true, nextFireAt: richTimestamp,
      }, {
        id: 'recurring', schedule: '', prompt: 'Repeat check',
        recurring: true, durable: false,
      }, {
        id: 'once', schedule: '', prompt: 'One shot check',
        recurring: false, durable: false,
      }],
    })
    const { rerender } = render(<TaskInspector view={view} />)

    expect(screen.getByText('Current objective')).toBeInTheDocument()
    expect(screen.getByText('Reported by the Loopal runtime')).toBeInTheDocument()
    expect(screen.getByText('Plan · 1/2')).toBeInTheDocument()
    expect(screen.getByText('Blocked by setup')).toBeInTheDocument()
    expect(screen.getByText('Blocks ship')).toBeInTheDocument()
    expect(screen.getByText('PASS')).toBeInTheDocument()
    expect(screen.getByText('Exit code 0')).toBeInTheDocument()
    expect(screen.getByText('0 * * * *')).toBeInTheDocument()
    expect(screen.getByText('Recurring')).toBeInTheDocument()
    expect(screen.getByText('One shot')).toBeInTheDocument()
    expect(screen.getByText(/Next ·/)).toBeInTheDocument()
    expect(screen.getAllByText('Exhausted')).toHaveLength(2)

    const empty = emptyView()
    rerender(<TaskInspector view={{ ...empty, tasks: view.tasks }} />)
    rerender(<TaskInspector view={{ ...empty, backgroundTasks: view.backgroundTasks }} />)
    rerender(<TaskInspector view={{ ...empty, crons: view.crons }} />)
    rerender(<TaskInspector view={empty} />)
    expect(screen.getByText('No Agent plan or background work yet.')).toBeInTheDocument()
    rerender(<TaskInspector view={undefined} />)
    expect(screen.getByText('No Agent plan or background work yet.')).toBeInTheDocument()
  })

  it('keeps goals read-only and controls only real runtime resources', () => {
    const onControl = vi.fn()
    const view = richView({
      backgroundTasks: [{
        id: 'live', description: 'Live command', status: 'running',
        exitCode: null, output: '', createdAt: richTimestamp,
      }, {
        id: 'done', description: 'Done command', status: 'completed',
        exitCode: 0, output: '', createdAt: richTimestamp,
      }],
    })
    const { rerender } = render(
      <TaskInspector view={view} canControl busy={false} onControl={onControl} />,
    )
    expect(screen.getByText('Active')).toBeInTheDocument()
    expect(screen.queryByRole('group', { name: /goal actions/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox', { name: /goal objective/i })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Kill background task Live command' }))
    fireEvent.click(screen.getByRole('button', { name: /Delete scheduled work/ }))
    expect(screen.queryByRole('button', { name: /Kill background task Done/ })).not.toBeInTheDocument()
    expect(onControl.mock.calls.map(([command]) => command)).toEqual([
      { type: 'background_task_kill', id: 'live' },
      { type: 'cron_delete', id: 'cron-health' },
    ])

    const { goal: _goal, ...withoutGoal } = view
    rerender(
      <TaskInspector view={withoutGoal} canControl busy={false} onControl={onControl} />,
    )
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
    rerender(<TaskInspector view={{ ...view, goal: { ...view.goal!, status: 'paused' } }} canControl busy={false} onControl={onControl} />)
    expect(screen.getByText('Paused')).toBeInTheDocument()
    rerender(<TaskInspector view={{ ...view, goal: { ...view.goal!, status: 'complete' } }} canControl busy={false} onControl={onControl} />)
    expect(screen.getByText('Complete')).toBeInTheDocument()
    expect(onControl).toHaveBeenCalledTimes(2)
  })
})

describe('DiagnosticsInspector', () => {
  it('renders Host, runtime, usage, degraded Hub, MCP data, and empty fallbacks', () => {
    const view = richView({
      hubDegradedSince: richTimestamp,
      mcpServers: [{
        name: 'broken', transport: 'stdio', source: 'workspace', status: 'failed',
        toolCount: 1, resourceCount: 2, promptCount: 3, errors: ['connection lost'],
      }],
    })
    const detail = richDetail({ agents: [richAgent({
      thinkingConfig: 'adaptive', permissionMode: 'ask', decisionMode: 'auto',
      sandboxPolicy: 'workspace-write', error: 'root broke',
    })], view })
    const { rerender } = render(<DiagnosticsInspector hostStatus="ready" detail={detail} />)

    expect(screen.getByText('ready')).toBeInTheDocument()
    expect(screen.getByText(/Hub degraded since/)).toBeInTheDocument()
    expect(screen.getByText('Agent failed: root broke')).toBeInTheDocument()
    expect(screen.getByText('Runtime configuration')).toBeInTheDocument()
    expect(screen.getByText('Usage')).toBeInTheDocument()
    expect(screen.getByText('0 active · 2 total')).toBeInTheDocument()
    expect(screen.getByText('MCP servers · 1')).toBeInTheDocument()
    expect(screen.getByText('connection lost')).toBeInTheDocument()

    rerender(<DiagnosticsInspector hostStatus="alive" detail={richDetail({
      agents: [{ id: 'worker', name: 'Worker', status: 'idle' }],
      view: emptyView(),
    })} />)
    expect(screen.getByText('No MCP servers reported.')).toBeInTheDocument()
    expect(screen.queryByText('Usage')).not.toBeInTheDocument()

    const { view: _view, ...withoutView } = richDetail({ agents: [] })
    rerender(<DiagnosticsInspector hostStatus="stopped" detail={withoutView} />)
    expect(screen.getByText('—')).toBeInTheDocument()
    rerender(<DiagnosticsInspector hostStatus="crashed" detail={undefined} />)
    expect(screen.getByText('crashed')).toBeInTheDocument()
  })

  it('controls MCP status for the selected live agent', () => {
    const onControl = vi.fn()
    const selectedView = richView({ mcpServers: [{
      name: 'ready-server', transport: 'stdio', source: 'builtin', status: 'ready',
      toolCount: 1, resourceCount: 0, promptCount: 0, errors: [],
    }, {
      name: 'offline-server', transport: 'http', source: 'workspace', status: 'failed',
      toolCount: 0, resourceCount: 0, promptCount: 0, errors: [],
    }] })
    const detail = richDetail({
      agents: [richAgent(), richAgent({
        id: 'child', name: 'Child', model: 'child-model', parentId: 'main', view: selectedView,
      })],
      view: richView({ mcpServers: [] }),
    })
    render(
      <DiagnosticsInspector
        hostStatus="ready" detail={detail} agentId="child"
        canControl busy={false} onControl={onControl}
      />,
    )
    expect(screen.getByText('child-model')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Refresh MCP status' }))
    fireEvent.click(screen.getByText('ready-server'))
    fireEvent.click(screen.getByText('offline-server'))
    fireEvent.click(screen.getByRole('button', { name: 'Disconnect MCP server ready-server' }))
    fireEvent.click(screen.getByRole('button', { name: 'Reconnect MCP server offline-server' }))
    expect(onControl.mock.calls.map(([command]) => command)).toEqual([
      { type: 'mcp_status' },
      { type: 'mcp_disconnect', server: 'ready-server' },
      { type: 'mcp_reconnect', server: 'offline-server' },
    ])
  })
})

function emptyView(): SessionView {
  const { goal: _goal, hubDegradedSince: _degraded, ...view } = richView()
  return {
    ...view, tasks: [], backgroundTasks: [], crons: [], mcpServers: [],
    historyTruncated: false, retryBanner: null, compactBanner: null,
  }
}
