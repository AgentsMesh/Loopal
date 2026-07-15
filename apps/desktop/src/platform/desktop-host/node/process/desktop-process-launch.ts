import { spawn, type ChildProcessByStdio } from 'node:child_process'
import { type Readable } from 'node:stream'
import { DeferredPromise } from '../../../../base/common/async'
import { type MetaHubStartupOptions } from '../host/desktop-host-types'

export type DesktopChild = ChildProcessByStdio<null, Readable, Readable>
export type SpawnDesktopProcess = (
  binaryPath: string,
  cwd: string,
  parentPid: number,
  env?: NodeJS.ProcessEnv,
  resumeSessionId?: string,
  metaHub?: MetaHubStartupOptions,
) => DesktopChild

export function spawnDesktopProcess(
  binaryPath: string,
  cwd: string,
  parentPid: number,
  env?: NodeJS.ProcessEnv,
  resumeSessionId?: string,
  metaHubOrSpawn?: MetaHubStartupOptions | typeof spawn,
  spawnFn: typeof spawn = spawn,
): DesktopChild {
  const metaHub = typeof metaHubOrSpawn === 'function' ? undefined : metaHubOrSpawn
  const spawnProcess = typeof metaHubOrSpawn === 'function' ? metaHubOrSpawn : spawnFn
  const args = ['desktop', 'serve', '--parent-pid', String(parentPid)]
  const sessionId = validateResumeSessionId(resumeSessionId)
  if (sessionId) args.push('--resume', sessionId)
  if (metaHub) args.push('--join-hub', metaHub.address, '--hub-name', metaHub.hubName)
  return spawnProcess(binaryPath, args, {
    cwd,
    env: {
      ...process.env,
      ...env,
      ...(metaHub ? { LOOPAL_META_HUB_TOKEN: metaHub.token } : {}),
    },
    shell: false,
    windowsHide: true,
    stdio: ['ignore', 'pipe', 'pipe'],
  })
}

export function validateResumeSessionId(value: string | undefined): string | undefined {
  if (value === undefined) return undefined
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(value)) {
    throw new Error('Invalid Loopal Desktop resume session ID')
  }
  return value
}

export async function withTimeout<T>(
  promise: Promise<T>, milliseconds: number, message: string,
): Promise<T> {
  const timeout = new DeferredPromise<T>()
  const timer = setTimeout(() => timeout.reject(new Error(message)), milliseconds)
  try {
    return await Promise.race([promise, timeout.promise])
  } finally {
    clearTimeout(timer)
  }
}
