import {
  keepE2eWindowHidden,
  resolveDesktopCwd,
  resolveLoopalBinary,
  resolveRendererUrl,
  useFakeBackend,
} from './runtime-mode'
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, win32 } from 'node:path'

describe('packaged runtime boundaries', () => {
  const temporaryDirectories: string[] = []
  const env = {
    LOOPAL_DESKTOP_BACKEND: 'fake',
    LOOPAL_DESKTOP_BINARY: '/tmp/other-loopal',
    LOOPAL_DESKTOP_CWD: '/tmp/other-workspace',
    LOOPAL_DESKTOP_E2E_HIDDEN: '1',
    ELECTRON_RENDERER_URL: 'https://attacker.invalid',
  }

  afterEach(() => {
    for (const directory of temporaryDirectories.splice(0)) {
      rmSync(directory, { recursive: true, force: true })
    }
  })

  it('allows explicit development overrides', () => {
    expect(useFakeBackend(false, env)).toBe(true)
    expect(keepE2eWindowHidden(false, env)).toBe(true)
    expect(resolveDesktopCwd(false, env, '/cwd')).toBe('/tmp/other-workspace')
    expect(resolveRendererUrl(false, env)).toBe('https://attacker.invalid')
    expect(resolveLoopalBinary({
      isPackaged: false,
      env,
      resourcesPath: '/resources',
      platform: 'darwin',
      cwd: '/cwd',
    })).toBe('/tmp/other-loopal')
  })

  it('ignores all development overrides in a packaged application', () => {
    expect(useFakeBackend(true, env)).toBe(false)
    expect(keepE2eWindowHidden(true, env)).toBe(false)
    expect(resolveDesktopCwd(true, env, '/cwd')).toBeUndefined()
    expect(resolveRendererUrl(true, env)).toBeUndefined()
    expect(resolveLoopalBinary({
      isPackaged: true,
      env,
      resourcesPath: '/resources',
      platform: 'darwin',
      cwd: '/cwd',
    })).toBe('/resources/bin/loopal')
    expect(resolveLoopalBinary({
      isPackaged: true,
      env: {},
      resourcesPath: 'C:/resources',
      platform: 'win32',
      cwd: 'C:/cwd',
    })).toBe(win32.join('C:/resources', 'bin', 'loopal.exe'))
  })

  it('falls back when development overrides are absent', () => {
    expect(useFakeBackend(false, {})).toBe(false)
    expect(keepE2eWindowHidden(false, {})).toBe(false)
    expect(resolveDesktopCwd(false, {}, '/cwd')).toBe('/cwd')
    expect(resolveRendererUrl(false, {})).toBeUndefined()
    expect(resolveLoopalBinary({
      isPackaged: false,
      env: {},
      resourcesPath: '/resources',
      platform: 'linux',
      cwd: '/cwd',
    })).toBeUndefined()
  })

  it('resolves a Bazel sidecar independently of the workspace cwd', () => {
    const runfiles = temporaryDirectory()
    const binary = join(runfiles, '_main', 'loopal')
    mkdirSync(join(runfiles, '_main'))
    writeFileSync(binary, '')
    expect(resolveLoopalBinary({
      isPackaged: false,
      env: {
        LOOPAL_DESKTOP_BINARY_RUNFILE: '_main/loopal',
        JS_BINARY__RUNFILES: runfiles,
      },
      resourcesPath: '/resources',
      platform: 'darwin',
      cwd: '/unrelated/workspace',
    })).toBe(binary)
  })

  it('resolves manifest-only runfiles and relative explicit overrides', () => {
    const directory = temporaryDirectory()
    const manifest = join(directory, 'MANIFEST')
    const binary = join(directory, 'physical loopal')
    writeFileSync(manifest, ` _main/loopal ${binary.replaceAll(' ', '\\s')}\n`)
    expect(resolveLoopalBinary({
      isPackaged: false,
      env: {
        LOOPAL_DESKTOP_BINARY_RUNFILE: '_main/loopal',
        RUNFILES_MANIFEST_FILE: manifest,
      },
      resourcesPath: '/resources', platform: 'linux', cwd: '/workspace',
    })).toBe(binary)
    expect(resolveLoopalBinary({
      isPackaged: false,
      env: { LOOPAL_DESKTOP_BINARY: './bin/loopal' },
      resourcesPath: '/resources', platform: 'linux', cwd: '/workspace',
    })).toBe('/workspace/bin/loopal')
  })

  it('handles absolute, missing, malformed, and unreadable runfiles metadata', () => {
    const directory = temporaryDirectory()
    const manifest = join(directory, 'MANIFEST')
    const binary = join(directory, 'loopal')
    writeFileSync(manifest, `malformed\n_main/loopal ${binary}\n`)
    const base = {
      isPackaged: false, resourcesPath: '/resources', platform: 'linux' as const, cwd: '/cwd',
    }
    expect(resolveLoopalBinary({
      ...base, env: { LOOPAL_DESKTOP_BINARY_RUNFILE: binary },
    })).toBe(binary)
    expect(resolveLoopalBinary({
      ...base, env: {
        LOOPAL_DESKTOP_BINARY_RUNFILE: '_main/loopal', RUNFILES_MANIFEST_FILE: manifest,
      },
    })).toBe(binary)
    expect(resolveLoopalBinary({
      ...base, env: {
        LOOPAL_DESKTOP_BINARY_RUNFILE: '_main/other', RUNFILES_MANIFEST_FILE: manifest,
      },
    })).toBeUndefined()
    expect(resolveLoopalBinary({
      ...base, env: {
        LOOPAL_DESKTOP_BINARY_RUNFILE: '_main/missing', RUNFILES_DIR: directory,
      },
    })).toBe(join(directory, '_main/missing'))
    expect(resolveLoopalBinary({
      ...base, env: {
        LOOPAL_DESKTOP_BINARY_RUNFILE: '_main/missing', TEST_SRCDIR: directory,
        RUNFILES_MANIFEST_FILE: join(directory, 'absent'),
      },
    })).toBe(join(directory, '_main/missing'))
  })

  function temporaryDirectory(): string {
    const directory = mkdtempSync(join(tmpdir(), 'loopal-runtime-mode-'))
    temporaryDirectories.push(directory)
    return directory
  }
})
