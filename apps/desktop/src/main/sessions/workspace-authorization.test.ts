import { mkdtemp, mkdir, readFile, realpath, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, parse } from 'node:path'
import {
  authorizePackagedWorkspace,
  workspaceRecordName,
  type WorkspaceAuthorizationOptions,
} from './workspace-authorization'

describe('packaged workspace authorization', () => {
  let root: string
  let home: string
  let userData: string
  let application: string
  let workspace: string

  beforeEach(async () => {
    root = await mkdtemp(join(tmpdir(), 'loopal-workspace-'))
    home = join(root, 'homes', 'stone')
    userData = join(root, 'state')
    application = join(root, 'install', 'Loopal.app')
    workspace = join(home, 'projects', 'loopal')
    await Promise.all([
      mkdir(userData, { recursive: true }),
      mkdir(join(application, 'internal'), { recursive: true }),
      mkdir(workspace, { recursive: true }),
    ])
  })

  afterEach(async () => {
    await rm(root, { recursive: true, force: true })
  })

  it('uses a valid stored canonical workspace without opening the chooser', async () => {
    await writeRecord(workspace)
    const selectDirectory = vi.fn<() => Promise<string | undefined>>()

    await expect(authorize({ selectDirectory })).resolves.toEqual({
      ok: true,
      path: await realpath(workspace),
    })
    expect(selectDirectory).not.toHaveBeenCalled()
  })

  it('reselects when a stored record is invalid or unsafe', async () => {
    const records = [
      'not json',
      'null',
      '[]',
      JSON.stringify({ version: 2, path: workspace }),
      JSON.stringify({ version: 1, path: 42 }),
      JSON.stringify({ version: 1, path: join(root, 'missing') }),
      JSON.stringify({ version: 1, path: application }),
    ]
    for (const record of records) {
      await writeFile(join(userData, workspaceRecordName), record)
      const selectDirectory = vi.fn(async () => workspace)
      const result = await authorize({ selectDirectory })
      expect(result).toEqual({ ok: true, path: await realpath(workspace) })
      expect(selectDirectory).toHaveBeenCalledOnce()
    }
  })

  it('fails closed when selection is cancelled or throws', async () => {
    const cancelled = await authorize({ selectDirectory: async () => undefined })
    expect(cancelled).toEqual({ ok: false, reason: 'Workspace selection was cancelled.' })

    const failed = await authorize({ selectDirectory: async () => {
      throw new Error('dialog unavailable')
    } })
    expect(failed).toEqual({ ok: false, reason: 'Workspace selection failed.' })
    await expect(readFile(join(userData, workspaceRecordName))).rejects.toThrow()
  })

  it('rejects roots, app-related paths, internal state, and files', async () => {
    const file = join(home, 'file.txt')
    await writeFile(file, 'not a directory')
    const dangerous = [
      parse(root).root,
      join(root, 'homes'),
      home,
      join(root, 'install'),
      application,
      join(application, 'internal'),
      userData,
      join(userData, 'nested'),
      file,
    ]
    await mkdir(join(userData, 'nested'))
    for (const path of dangerous) {
      const result = await authorize({ selectDirectory: async () => path })
      expect(result).toEqual({
        ok: false,
        reason: 'The selected workspace is unavailable or unsafe.',
      })
    }
  })

  it('atomically persists a successful explicit selection', async () => {
    const result = await authorize({ selectDirectory: async () => workspace })
    const canonical = await realpath(workspace)

    expect(result).toEqual({ ok: true, path: canonical })
    expect(JSON.parse(await readFile(join(userData, workspaceRecordName), 'utf8'))).toEqual({
      version: 1,
      path: canonical,
    })
    expect((await readdir(userData)).filter((name) => name.endsWith('.tmp'))).toEqual([])
  })

  it('fails closed when the authorization record cannot be persisted', async () => {
    await mkdir(join(userData, workspaceRecordName))

    await expect(authorize({
      selectDirectory: async () => workspace,
    })).resolves.toEqual({
      ok: false,
      reason: 'The workspace authorization could not be saved.',
    })
    expect((await readdir(userData)).filter((name) => name.endsWith('.tmp'))).toEqual([])
  })

  function authorize(
    overrides: Partial<WorkspaceAuthorizationOptions>,
  ) {
    return authorizePackagedWorkspace({
      userDataPath: userData,
      homePath: home,
      applicationPaths: [application, join(root, 'missing-app')],
      selectDirectory: async () => undefined,
      ...overrides,
    })
  }

  async function writeRecord(path: string): Promise<void> {
    await writeFile(
      join(userData, workspaceRecordName),
      JSON.stringify({ version: 1, path }),
    )
  }
})
