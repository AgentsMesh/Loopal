import { fireEvent, render, screen } from '@testing-library/react'
import { type Artifact } from '../../../../shared/contracts'
import { ArtifactPanel } from './artifact-panel'

const artifact: Artifact = {
  id: 'report', sessionId: 'session', title: 'Report.md', kind: 'report',
  uri: 'loopal-artifact://report', mediaType: 'text/markdown',
  producerAgentId: 'main', createdAt: '2026-07-11T12:00:00.000Z',
}

describe('ArtifactPanel', () => {
  it('expands, switches, and collapses artifact metadata', () => {
    render(<ArtifactPanel artifacts={[
      artifact,
      { ...artifact, id: 'code', title: 'main.rs', kind: 'code', uri: 'file:///main.rs' },
    ]} />)
    const report = screen.getByRole('button', { name: /Report.md/ })
    const code = screen.getByRole('button', { name: /main.rs/ })
    fireEvent.click(report)
    expect(report).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByText('loopal-artifact://report')).toBeInTheDocument()
    fireEvent.click(code)
    expect(report).toHaveAttribute('aria-expanded', 'false')
    expect(screen.getByText('file:///main.rs')).toBeInTheDocument()
    fireEvent.click(code)
    expect(code).toHaveAttribute('aria-expanded', 'false')
  })

  it('renders an explicit empty state', () => {
    render(<ArtifactPanel artifacts={[]} />)
    expect(screen.getByText(/Artifacts produced by this session/)).toBeInTheDocument()
  })
})
