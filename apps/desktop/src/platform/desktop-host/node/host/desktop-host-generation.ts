import { DisposableStore } from '../../../../base/common/lifecycle'
import {
  DesktopProcess, DesktopProcessTerminationError, withTimeout,
} from '../process/desktop-process'
import { type DesktopHostSession } from './desktop-host-types'
import { type JsonRpcClient } from '../rpc/jsonrpc-client'

export interface DesktopHostGeneration {
  readonly command: number
  readonly process: DesktopProcess
  readonly subscriptions: DisposableStore
  rpc?: JsonRpcClient
  session?: DesktopHostSession
  cleanup?: Promise<void>
  closing: boolean
  exited: boolean
}

export function createGeneration(
  command: number,
  process: DesktopProcess,
): DesktopHostGeneration {
  return {
    command,
    process,
    subscriptions: new DisposableStore(),
    closing: false,
    exited: false,
  }
}

export function terminateGeneration(
  generation: DesktopHostGeneration,
  graceful: boolean,
  timeoutMs: number,
): Promise<void> {
  generation.cleanup ??= terminateInternal(generation, graceful, timeoutMs)
  return generation.cleanup
}

async function terminateInternal(
  generation: DesktopHostGeneration,
  graceful: boolean,
  timeoutMs: number,
): Promise<void> {
  generation.closing = true
  delete generation.session
  generation.subscriptions.dispose()
  try {
    let sentTerm = !graceful || !generation.rpc
    if (!sentTerm) {
      try {
        await withTimeout(
          generation.rpc!.call('hub/shutdown', {}),
          timeoutMs,
          'Loopal Hub shutdown request timed out',
        )
      } catch {
        sentTerm = true
      }
    }
    if (sentTerm && !generation.exited) generation.process.kill('SIGTERM')
    const exit = generation.process.waitForExit()
    try {
      await withTimeout(exit, timeoutMs, 'Loopal Desktop Host did not exit after shutdown')
      generation.exited = true
    } catch {
      generation.process.kill('SIGKILL')
      try {
        await withTimeout(
          exit,
          Math.max(timeoutMs, 100),
          'Loopal Desktop Host stdout did not close after SIGKILL',
        )
        generation.exited = true
      } catch {
        if (!generation.process.forceFinalizeProtocol()) {
          throw new DesktopProcessTerminationError(
            'desktop_process_termination_unconfirmed: Host did not exit after SIGKILL',
          )
        }
        const forced = await exit
        generation.exited = true
        throw forced.error
      }
    }
  } finally {
    generation.rpc?.dispose()
    delete generation.rpc
  }
}
