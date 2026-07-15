import { EventEmitter } from 'node:events'
import { vi } from 'vitest'

export class FakeSocket extends EventEmitter {
  readonly writes: string[] = []
  readonly destroyReasons: Array<Error | undefined> = []
  readonly setEncoding = vi.fn(() => this)
  readonly end = vi.fn(() => this)
  throwOnWrite: unknown

  write(value: string): boolean {
    if (this.throwOnWrite !== undefined) {
      throw this.throwOnWrite
    }
    this.writes.push(value)
    return true
  }

  destroy(reason?: Error): this {
    this.destroyReasons.push(reason)
    return this
  }

  data(value: string | Buffer): void {
    this.emit('data', value)
  }

  close(): void {
    this.emit('close')
  }

  fail(error: Error): void {
    this.emit('error', error)
  }
}

export function response(id: number, result: unknown): string {
  return `${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`
}
