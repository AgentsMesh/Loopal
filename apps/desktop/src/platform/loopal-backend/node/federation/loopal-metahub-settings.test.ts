import { mkdtemp, readFile, rm, stat } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { LoopalMetaHubSettings } from './loopal-metahub-settings'

describe('LoopalMetaHubSettings', () => {
  let root = ''
  let path = ''

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), 'loopal-metahub-settings-'))
    path = join(root, 'nested', 'metahub.json')
  })
  afterEach(async () => rm(root, { recursive: true, force: true }))

  it('persists credentials privately without returning the token', async () => {
    const settings = new LoopalMetaHubSettings(path)
    await settings.load()
    const publicValue = await settings.update({
      address: '127.0.0.1:9000', hubName: 'desktop-a', joinOnStart: true,
      startLocalOnLaunch: false, token: 'cluster-secret',
    })
    expect(publicValue).toEqual({
      address: '127.0.0.1:9000', hubName: 'desktop-a', joinOnStart: true,
      startLocalOnLaunch: false, tokenConfigured: true,
    })
    expect(JSON.stringify(publicValue)).not.toContain('cluster-secret')
    expect(settings.startup).toEqual({
      address: '127.0.0.1:9000', hubName: 'desktop-a', token: 'cluster-secret',
    })
    if (process.platform !== 'win32') expect((await stat(path)).mode & 0o777).toBe(0o600)

    const restored = new LoopalMetaHubSettings(path)
    await restored.load()
    expect(restored.publicValue.tokenConfigured).toBe(true)
    expect(await readFile(path, 'utf8')).toContain('cluster-secret')
  })

  it('does not join a runtime merely because the managed coordinator starts', async () => {
    const settings = new LoopalMetaHubSettings()
    await settings.update({
      address: '', hubName: 'desktop-local', joinOnStart: false,
      startLocalOnLaunch: true, token: 'old',
    })
    await settings.useManaged('127.0.0.1:3456', 'managed-secret')
    expect(settings.startup).toBeUndefined()
    await settings.update({
      address: '127.0.0.1:3456', hubName: 'desktop-local', joinOnStart: false,
      startLocalOnLaunch: false, clearToken: true,
    })
    expect(settings.publicValue.tokenConfigured).toBe(false)
    expect(settings.startup).toBeUndefined()
  })

  it('clears only the active managed credential and disables stale startup', async () => {
    const settings = new LoopalMetaHubSettings(path)
    await settings.update({
      address: '127.0.0.1:4567', hubName: 'desktop-local', joinOnStart: true,
      startLocalOnLaunch: true, token: 'managed-secret',
    })
    expect(await settings.clearManaged('127.0.0.1:9999')).toBe(false)
    expect(await settings.clearManaged('127.0.0.1:4567')).toBe(true)
    expect(settings.publicValue).toMatchObject({
      address: '', joinOnStart: false, startLocalOnLaunch: true, tokenConfigured: false,
    })
    expect(settings.startup).toBeUndefined()
    expect(await readFile(path, 'utf8')).not.toContain('managed-secret')

    await settings.useManaged('127.0.0.1:5000', 'local-secret')
    await settings.update({
      address: 'meta.example:9000', hubName: 'external', joinOnStart: true,
      startLocalOnLaunch: false, token: 'external-secret',
    })
    expect(await settings.clearManaged('127.0.0.1:5000')).toBe(false)
    expect(settings.credentials).toEqual({
      address: 'meta.example:9000', hubName: 'external', token: 'external-secret',
    })
  })
})
