import { type SpawnDesktopProcess } from '../process/desktop-process'
import { type JsonRpcClient } from '../rpc/jsonrpc-client'

export interface DesktopHostSession {
  readonly sessionId: string
  readonly serverVersion: string
  readonly pid: number
}
export type DesktopHostActivation = (session: DesktopHostSession) => Promise<void>

export interface MetaHubStartupOptions {
  readonly address: string
  readonly hubName: string
  readonly token: string
}

export interface DesktopHostOptions {
  readonly binaryPath: string
  readonly cwd: string
  readonly resumeSessionId?: string
  readonly parentPid?: number
  readonly env?: NodeJS.ProcessEnv
  readonly startupTimeoutMs?: number
  readonly shutdownTimeoutMs?: number
  readonly clientName?: string
  readonly spawnProcess?: SpawnDesktopProcess
  readonly connectRpc?: typeof JsonRpcClient.connect
  readonly metaHub?: MetaHubStartupOptions
}
