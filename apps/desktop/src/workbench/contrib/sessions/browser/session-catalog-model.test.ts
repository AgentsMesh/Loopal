import { type SessionSummary } from '../../../../shared/contracts'
import { preferredSessionId, sessionCatalogModel } from './session-catalog-model'

const base: SessionSummary = {
  id: 'base', workspaceId: 'workspace-a', title: 'Base', model: 'gpt-5', mode: 'act',
  status: 'running', activeRuntimeId: 'runtime-base',
  createdAt: '2026-07-10T00:00:00.000Z', updatedAt: '2026-07-10T00:00:00.000Z',
}

describe('sessionCatalogModel', () => {
  it('projects recent live sessions across workspaces and searches the full history', () => {
    const olderLive = { ...base, id: 'live-a', title: 'Live alpha' }
    const newerLive = {
      ...base, id: 'live-b', workspaceId: 'workspace-b', title: 'Live beta',
      updatedAt: '2026-07-12T00:00:00.000Z', activeRuntimeId: 'runtime-b',
    }
    const stopped = {
      ...base, id: 'history-a', title: 'Legacy investigation', status: 'stopped' as const,
      activeRuntimeId: undefined, updatedAt: '2026-07-13T00:00:00.000Z',
    }
    const archived = {
      ...base, id: 'history-b', workspaceId: 'workspace-b', title: 'Legacy release',
      status: 'archived' as const, activeRuntimeId: undefined,
      updatedAt: '2026-07-11T00:00:00.000Z',
    }
    const sessions = [olderLive, stopped, newerLive, archived]

    expect(sessionCatalogModel(sessions, '').currentSessions.map(({ id }) => id))
      .toEqual(['live-b', 'live-a'])
    expect(sessionCatalogModel(sessions, '').searchResults).toEqual([])
    expect(sessionCatalogModel(sessions, ' legacy ').searchResults.map(({ id }) => id))
      .toEqual(['history-a', 'history-b'])
  })

  it('prefers an explicit active session, then the newest live session only', () => {
    const history = { ...base, id: 'history', status: 'stopped' as const,
      activeRuntimeId: undefined, updatedAt: '2026-07-13T00:00:00.000Z' }
    const live = { ...base, id: 'live', updatedAt: '2026-07-12T00:00:00.000Z' }
    expect(preferredSessionId([history, live], history.id)).toBe(history.id)
    expect(preferredSessionId([history, live])).toBe(live.id)
    expect(preferredSessionId([history])).toBeUndefined()
  })
})
