import { createInterface, type Interface } from 'node:readline'
import { Emitter, type Event } from '../../../../base/common/event'
import { parseDesktopHandshakeLine } from '../../common/desktop-handshake'
import { type MetaHubStartupOptions } from '../host/desktop-host-types'
import {
  type DesktopChild, type SpawnDesktopProcess, validateResumeSessionId,
} from './desktop-process-launch'
import { DesktopProcessPhases } from './desktop-process-phases'

export {
  spawnDesktopProcess, validateResumeSessionId, withTimeout,
} from './desktop-process-launch'
export type { DesktopChild, SpawnDesktopProcess } from './desktop-process-launch'

export interface DesktopProcessOptions {
  readonly binaryPath: string
  readonly cwd: string
  readonly parentPid: number
  readonly env?: NodeJS.ProcessEnv | undefined
  readonly resumeSessionId?: string | undefined
  readonly spawnProcess: SpawnDesktopProcess
  readonly metaHub?: MetaHubStartupOptions
}

export interface DesktopProcessExit {
  readonly code: number | null
  readonly signal: NodeJS.Signals | null
  readonly error: Error
}

export class DesktopProtocolDrainError extends Error {
  readonly code = 'desktop_protocol_drain_incomplete'
}

export class DesktopProcessTerminationError extends Error {
  readonly code = 'desktop_process_termination_unconfirmed'
}

export class DesktopProcess {
  private readonly phases = new DesktopProcessPhases()
  private readonly exitEmitter = new Emitter<DesktopProcessExit>()
  private readonly child: DesktopChild
  private readonly stdout: Interface
  private readonly diagnosticLines: string[] = []
  private exit: DesktopProcessExit | undefined
  private pendingExit: DesktopProcessExit | undefined
  private protocolClosed = false
  private childClosed = false
  private exitObserved = false
  private aliveObserved = false
  private sessionReported = false

  readonly alive = this.phases.alive
  readonly sessionCreated = this.phases.sessionCreated
  readonly ready = this.phases.ready
  readonly onExit: Event<DesktopProcessExit> = this.exitEmitter.event

  constructor(options: DesktopProcessOptions) {
    const base = [
      options.binaryPath, options.cwd, options.parentPid, options.env,
      validateResumeSessionId(options.resumeSessionId),
    ] as const
    this.child = options.metaHub
      ? options.spawnProcess(...base, options.metaHub)
      : options.spawnProcess(...base)
    this.stdout = createInterface({ input: this.child.stdout, crlfDelay: Infinity })
    this.stdout.on('line', (line) => this.acceptLine(line, options.parentPid))
    this.stdout.once('close', () => this.finishProtocol())
    this.child.stderr.setEncoding('utf8')
    this.child.stderr.on('data', (value: string | Buffer) => {
      this.captureDiagnostics(value.toString())
    })
    this.child.once('error', (error) => this.observeExit(null, null, error))
    this.child.once('exit', (code, signal) => {
      this.exitObserved = true
      this.observeExit(code, signal)
    })
    this.child.once('close', (code, signal) => this.finishChild(code, signal))
  }

  get diagnostics(): readonly string[] {
    return this.diagnosticLines
  }

  get creationMayHaveCommitted(): boolean {
    return this.aliveObserved && !this.sessionReported
  }

  get didReportSession(): boolean {
    return this.sessionReported
  }

  kill(signal: NodeJS.Signals): boolean {
    return this.child.kill(signal)
  }

  waitForExit(): Promise<DesktopProcessExit> {
    if (this.exit) return Promise.resolve(this.exit)
    return new Promise((resolve) => {
      const subscription = this.onExit((exit) => {
        subscription.dispose()
        resolve(exit)
      })
    })
  }

  forceFinalizeProtocol(): boolean {
    if (this.exit) return true
    const previous = this.pendingExit
    if (!previous || !this.exitObserved) return false
    this.pendingExit = {
      code: previous.code,
      signal: previous.signal,
      error: new DesktopProtocolDrainError(
        'desktop_protocol_drain_incomplete: stdout did not close after SIGKILL', {
        cause: previous.error,
      }),
    }
    this.childClosed = true
    this.protocolClosed = true
    this.stdout.close()
    this.child.stdout.destroy()
    this.child.stderr.destroy()
    this.finishExit()
    return true
  }

  private acceptLine(line: string, parentPid: number): void {
    try {
      const handshake = parseDesktopHandshakeLine(line)
      if (!handshake) return
      if (handshake.pid !== this.child.pid || handshake.parent_pid !== parentPid) {
        throw new Error('Loopal Desktop Host handshake PID metadata does not match its process')
      }
      if (this.phases.accept(handshake)) {
        if (handshake.phase === 'alive') this.aliveObserved = true
        if (handshake.phase === 'session_created' || handshake.phase === 'ready') {
          this.sessionReported = true
        }
      }
    } catch (error) {
      this.phases.reject(error)
    }
  }

  private captureDiagnostics(chunk: string): void {
    for (const line of chunk.split(/\r?\n/)) {
      if (!line) continue
      this.diagnosticLines.push(line.slice(0, 2_000))
      if (this.diagnosticLines.length > 100) this.diagnosticLines.shift()
    }
  }

  private observeExit(
    code: number | null,
    signal: NodeJS.Signals | null,
    cause?: Error,
  ): void {
    if (this.exit || this.pendingExit) return
    const error = cause ?? new Error(
      `Loopal Desktop Host exited before shutdown (code=${String(code)}, signal=${String(signal)})`,
    )
    this.pendingExit = { code, signal, error }
  }

  private finishProtocol(): void {
    this.protocolClosed = true
    this.finishExit()
  }

  private finishChild(code: number | null, signal: NodeJS.Signals | null): void {
    this.childClosed = true
    this.observeExit(code, signal)
    this.finishExit()
  }

  private finishExit(): void {
    if (this.exit || !this.pendingExit || !this.protocolClosed || !this.childClosed) return
    this.exit = this.pendingExit
    this.pendingExit = undefined
    const { error } = this.exit
    this.phases.reject(error)
    this.exitEmitter.fire(this.exit)
  }
}
