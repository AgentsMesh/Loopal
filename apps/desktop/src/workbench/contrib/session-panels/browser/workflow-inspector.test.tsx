import { render, screen } from '@testing-library/react'
import {
  richTimestamp, richView,
} from '../../../../../test/fixtures/workbench/rich-session'
import { WorkflowInspector } from './workflow-inspector'

describe('WorkflowInspector', () => {
  it('renders active and recent workflow state from the session projection', () => {
    const run = {
      id: 'wrun_ui', runGoal: 'Publish release', state: 'running' as const,
      revision: 7, outputNode: 'publish', createdAt: richTimestamp, updatedAt: richTimestamp,
      counts: {
        pending: 1, ready: 1, active: 1, succeeded: 2,
        failed: 0, cancelled: 0, skipped: 0,
      },
    }
    render(<WorkflowInspector view={richView({ workflows: {
      active: [run], recent: [{ ...run, id: 'wrun_done', runGoal: 'Completed release',
        state: 'succeeded', revision: 8 }],
    } })} />)
    expect(screen.getByText('Active workflows')).toBeInTheDocument()
    expect(screen.getByText('Recent workflows')).toBeInTheDocument()
    expect(screen.getByText('Publish release')).toBeInTheDocument()
    expect(screen.getAllByText('Nodes · 2/5 complete')).toHaveLength(2)
    expect(screen.getByText('wrun_ui · revision 7')).toBeInTheDocument()
    expect(screen.getByText('Completed release')).toBeInTheDocument()
  })
})
