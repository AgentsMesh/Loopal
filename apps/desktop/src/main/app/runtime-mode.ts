import { existsSync, readFileSync } from 'node:fs'
import { isAbsolute, join, posix, win32 } from 'node:path'

export function useFakeBackend(isPackaged: boolean, env: NodeJS.ProcessEnv): boolean {
  return !isPackaged && env.LOOPAL_DESKTOP_BACKEND === 'fake'
}

export function keepE2eWindowHidden(isPackaged: boolean, env: NodeJS.ProcessEnv): boolean {
  return !isPackaged && env.LOOPAL_DESKTOP_E2E_HIDDEN === '1'
}

export function resolveDesktopCwd(
  isPackaged: boolean,
  env: NodeJS.ProcessEnv,
  cwd: string,
): string | undefined {
  if (isPackaged) return undefined
  return env.LOOPAL_DESKTOP_CWD || cwd
}

export function resolveRendererUrl(
  isPackaged: boolean,
  env: NodeJS.ProcessEnv,
): string | undefined {
  return isPackaged ? undefined : env.ELECTRON_RENDERER_URL
}

export function resolveLoopalBinary(options: {
  readonly isPackaged: boolean
  readonly env: NodeJS.ProcessEnv
  readonly resourcesPath: string
  readonly platform: NodeJS.Platform
  readonly cwd: string
}): string | undefined {
  const platformPath = options.platform === 'win32' ? win32 : posix
  if (!options.isPackaged) {
    const override = options.env.LOOPAL_DESKTOP_BINARY
    if (override) {
      return platformPath.isAbsolute(override)
        ? override
        : platformPath.resolve(options.cwd, override)
    }
    return resolveRunfile(options.env.LOOPAL_DESKTOP_BINARY_RUNFILE, options.env)
  }
  const executable = options.platform === 'win32' ? 'loopal.exe' : 'loopal'
  return platformPath.join(options.resourcesPath, 'bin', executable)
}

function resolveRunfile(
  runfile: string | undefined,
  env: NodeJS.ProcessEnv,
): string | undefined {
  if (!runfile) return undefined
  if (isAbsolute(runfile)) return runfile
  const root = env.JS_BINARY__RUNFILES || env.RUNFILES_DIR || env.TEST_SRCDIR
  const candidate = root ? join(root, runfile) : undefined
  if (candidate && existsSync(candidate)) return candidate
  const manifest = env.RUNFILES_MANIFEST_FILE
  if (!manifest) return candidate
  try {
    for (const line of readFileSync(manifest, 'utf8').split(/\r?\n/u)) {
      const entry = parseManifestEntry(line)
      if (entry?.[0] === runfile) return entry[1]
    }
  } catch {
    return candidate
  }
  return candidate
}

function parseManifestEntry(line: string): readonly [string, string] | undefined {
  const escaped = line.startsWith(' ')
  const start = escaped ? 1 : 0
  const separator = line.indexOf(' ', start)
  if (separator < 0) return undefined
  const decode = (value: string): string => escaped
    ? value.replace(/\\([snb])/gu, (_, code: string) => ({ s: ' ', n: '\n', b: '\\' })[code]!)
    : value
  return [decode(line.slice(start, separator)), decode(line.slice(separator + 1))]
}
