import type { ChildProcess } from 'node:child_process'
import { createInterface } from 'node:readline'
import { HubProbe } from './hub-probe'

export { HubProbe } from './hub-probe'

const DESKTOP_PREFIX = 'LOOPAL_DESKTOP '
const HANDSHAKE_TIMEOUT_MS = 20_000

export function registerCapableUiBeforeReady(child: ChildProcess): Promise<HubProbe> {
  return new Promise((resolve, reject) => {
    const lines = createInterface({ input: child.stdout!, crlfDelay: Infinity })
    let probe: HubProbe | undefined
    let registrationStarted = false
    let ready = false
    let settled = false
    const timer = setTimeout(
      () => finish(new Error('Timed out waiting for remote Hub readiness')),
      HANDSHAKE_TIMEOUT_MS,
    )
    const cleanup = (): void => {
      clearTimeout(timer)
      lines.off('line', onLine)
      lines.close()
      child.off('exit', onExit)
      child.off('error', onError)
    }
    const finish = (error?: Error): void => {
      if (settled || (!error && (!probe || !ready))) return
      settled = true
      cleanup()
      if (error) {
        probe?.close()
        reject(error)
      } else {
        resolve(probe!)
      }
    }
    const register = async (value: Record<string, unknown>): Promise<void> => {
      if (registrationStarted) return
      registrationStarted = true
      try {
        const candidate = await HubProbe.connect(String(value.addr), String(value.token))
        if (settled) return candidate.close()
        probe = candidate
        finish()
      } catch (error) {
        finish(asError(error))
      }
    }
    const onLine = (line: string): void => {
      if (!line.startsWith(DESKTOP_PREFIX)) return
      try {
        const value = JSON.parse(line.slice(DESKTOP_PREFIX.length)) as Record<string, unknown>
        if (value.phase === 'alive') void register(value)
        if (value.phase === 'ready') { ready = true; finish() }
        if (value.phase === 'error') {
          finish(new Error(`Remote Hub startup failed: ${String(value.message ?? value.code)}`))
        }
      } catch (error) {
        finish(new Error(`Invalid remote Hub handshake: ${asError(error).message}`))
      }
    }
    const onExit = (): void => finish(new Error('Remote Hub exited before ready'))
    const onError = (error: Error): void => finish(error)
    child.once('exit', onExit)
    child.once('error', onError)
    lines.on('line', onLine)
  })
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error))
}
