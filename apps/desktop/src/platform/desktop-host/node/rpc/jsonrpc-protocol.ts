export interface JsonRpcNotification {
  readonly method: string
  readonly params: unknown
}

export interface JsonRpcClientOptions {
  readonly requestTimeoutMs?: number
  readonly maxFrameBytes?: number
}

export interface JsonRpcPendingRequest {
  readonly resolve: (value: unknown) => void
  readonly reject: (reason: unknown) => void
  readonly timer: ReturnType<typeof setTimeout>
  readonly removeAbortListener: () => void
}

export const MAX_DESKTOP_HOST_FRAME_BYTES = 64 * 1024 * 1024

export function encodeJsonRpcFrame(value: unknown, maxFrameBytes: number): string {
  const frame = JSON.stringify(value)
  if (Buffer.byteLength(frame) > maxFrameBytes) {
    throw new Error('Desktop attempted to send an oversized JSON-RPC frame')
  }
  return `${frame}\n`
}

export class JsonRpcRemoteError extends Error {
  constructor(
    readonly code: number,
    message: string,
    readonly data?: unknown,
  ) {
    super(message)
    this.name = 'JsonRpcRemoteError'
  }
}

export class JsonRpcFrameDecoder {
  private buffer = ''

  constructor(readonly maxFrameBytes: number) {}

  accept(chunk: string | Buffer): string[] {
    this.buffer += chunk.toString()
    if (Buffer.byteLength(this.buffer) > this.maxFrameBytes && !this.buffer.includes('\n')) {
      throw new Error('Loopal Hub sent an oversized JSON-RPC frame')
    }

    const frames: string[] = []
    let boundary = this.buffer.indexOf('\n')
    while (boundary >= 0) {
      const line = this.buffer.slice(0, boundary).replace(/\r$/, '')
      this.buffer = this.buffer.slice(boundary + 1)
      if (Buffer.byteLength(line) > this.maxFrameBytes) {
        throw new Error('Loopal Hub sent an oversized JSON-RPC frame')
      }
      if (line.length > 0) {
        frames.push(line)
      }
      boundary = this.buffer.indexOf('\n')
    }
    return frames
  }
}

export function parseTcpAddress(address: string): { host: string; port: number } {
  const separator = address.lastIndexOf(':')
  const host = address.slice(0, separator)
  const port = Number(address.slice(separator + 1))
  if (separator <= 0 || !host || !Number.isInteger(port) || port <= 0 || port > 65_535) {
    throw new Error(`Invalid Loopal Hub TCP address: ${address}`)
  }
  return { host, port }
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
