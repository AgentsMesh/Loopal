import { mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { DesktopPreferencesService } from './desktop-preferences-service'

describe('DesktopPreferencesService', () => {
  let root = ''
  let path = ''

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), 'loopal-desktop-preferences-'))
    path = join(root, 'nested', 'desktop-preferences.json')
  })
  afterEach(async () => rm(root, { recursive: true, force: true }))

  it('defaults to the system locale and restores an atomic private update', async () => {
    const service = new DesktopPreferencesService(path)
    await expect(service.getDesktopPreferences()).resolves.toEqual({ locale: 'system' })
    await expect(service.updateDesktopPreferences({ locale: 'zh-CN' })).resolves
      .toEqual({ locale: 'zh-CN' })
    if (process.platform !== 'win32') expect((await stat(path)).mode & 0o777).toBe(0o600)
    expect(JSON.parse(await readFile(path, 'utf8'))).toEqual({ version: 1, locale: 'zh-CN' })

    const restored = new DesktopPreferencesService(path)
    await expect(restored.getDesktopPreferences()).resolves.toEqual({ locale: 'zh-CN' })
  })

  it('repairs malformed persisted values and validates updates', async () => {
    await writeFile(path, '{"version":1,"locale":"invalid"}', { flag: 'w' }).catch(async () => {
      const service = new DesktopPreferencesService(path)
      await service.updateDesktopPreferences({ locale: 'en' })
      await writeFile(path, '{"version":1,"locale":"invalid"}')
    })
    const service = new DesktopPreferencesService(path)
    await expect(service.getDesktopPreferences()).resolves.toEqual({ locale: 'system' })
    await service.flush()
    expect(JSON.parse(await readFile(path, 'utf8'))).toEqual({ version: 1, locale: 'system' })
    await expect(service.updateDesktopPreferences({ locale: 'fr' } as never)).rejects.toThrow()
  })

  it('serializes concurrent updates and returns immutable snapshots', async () => {
    const service = new DesktopPreferencesService(path)
    const first = service.updateDesktopPreferences({ locale: 'en' })
    const second = service.updateDesktopPreferences({ locale: 'zh-CN' })
    await Promise.all([first, second])
    await service.flush()
    expect(await service.getDesktopPreferences()).toEqual({ locale: 'zh-CN' })
    expect(JSON.parse(await readFile(path, 'utf8')).locale).toBe('zh-CN')
  })
})
