import { render, screen } from '@testing-library/react'
import { sessionOne, sessionTwo } from '../../../test/support/workbench/api-stub'
import { type FederationSnapshot } from '../contrib/federation/browser/federation-model'
import { StatusBar } from './status-bar'

const federation: FederationSnapshot = {
  local: { state: 'stopped' },
  network: {
    state: 'disconnected', hubs: [], topology: [],
    refreshedAt: '2026-07-11T12:00:00.000Z',
  },
  connections: [], topology: [], memberships: {},
}

describe('StatusBar', () => {
  it('counts every live Runtime, including sessions waiting for input', () => {
    const stopped = {
      ...sessionOne, id: 'history', status: 'stopped' as const,
      activeRuntimeId: undefined,
    }
    render(<StatusBar sessions={[sessionOne, sessionTwo, stopped]}
      federation={federation} onOpenFederation={vi.fn()} />)

    expect(screen.getByText('2 running')).toBeInTheDocument()
  })
})
