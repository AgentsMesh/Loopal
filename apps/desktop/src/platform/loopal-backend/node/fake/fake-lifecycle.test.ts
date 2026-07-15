import { type SessionDetail } from '../../../../shared/contracts'
import { FakeDesktopBackend } from './fake-backend'

describe('FakeDesktopBackend lifecycle parity', () => {
  it('rejects sends after stop and restarts of archived sessions', async () => {
    const backend = new FakeDesktopBackend()
    const selected = await backend.authorizeSessionDirectory(process.cwd())
    const created = await backend.createSession({
      authorizationId: selected.authorizationId, launchMode: 'directory',
    })
    await backend.stopSession(created.session.id)
    await expect(backend.sendMessage(created.session.id, 'must not run'))
      .rejects.toThrow('restart it first')

    const internals = backend as unknown as {
      catalog: { details: Map<string, SessionDetail> }
    }
    const detail = internals.catalog.details.get(created.session.id)!
    detail.session = { ...detail.session, status: 'archived' }
    await expect(backend.restartSession(created.session.id))
      .rejects.toThrow('Archived session')
    backend.dispose()
  })
})
