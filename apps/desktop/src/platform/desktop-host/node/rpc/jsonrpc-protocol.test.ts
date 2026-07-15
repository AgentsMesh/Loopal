import { afterEach, describe, expect, it, vi } from 'vitest'
import { FakeSocket } from '../../../../../test/support/ipc/jsonrpc'

const net = vi.hoisted(() => ({ createConnection: vi.fn() }))

vi.mock('node:net', () => ({
  createConnection: net.createConnection,
  default: { createConnection: net.createConnection },
}))

import { JsonRpcClient } from './jsonrpc-client'

function createClient(options: ConstructorParameters<typeof JsonRpcClient>[1] = {}) {
  const socket = new FakeSocket()
  const client = new JsonRpcClient(socket as never, options)
  return { client, socket }
}

afterEach(() => {
  net.createConnection.mockReset()
  vi.useRealTimers()
})

describe('JsonRpcClient protocol', () => {
  it('destroys the socket for malformed, invalid, missing-id, and oversized frames', async () => {
    const malformed = createClient()
    malformed.socket.data('{not-json}\n')
    expect(malformed.socket.destroyReasons.at(-1)?.message).toContain('malformed JSON')
    malformed.client.dispose()

    for (const value of ['text', null, [], { jsonrpc: '1.0' }]) {
      const invalid = createClient()
      invalid.socket.data(`${JSON.stringify(value)}\n`)
      expect(invalid.socket.destroyReasons.at(-1)?.message).toContain('invalid JSON-RPC')
      invalid.client.dispose()
    }

    const missingId = createClient()
    missingId.socket.data(`${JSON.stringify({ jsonrpc: '2.0', result: true })}\n`)
    expect(missingId.socket.destroyReasons.at(-1)?.message).toContain('numeric id')
    missingId.client.dispose()

    const unterminated = createClient({ maxFrameBytes: 5 })
    unterminated.socket.data('123456')
    expect(unterminated.socket.destroyReasons.at(-1)?.message).toContain('oversized')
    unterminated.client.dispose()

    const terminated = createClient({ maxFrameBytes: 5 })
    terminated.socket.data('123456\n')
    expect(terminated.socket.destroyReasons.at(-1)?.message).toContain('oversized')
    terminated.client.dispose()

    const outbound = createClient({ maxFrameBytes: 5 })
    await expect(outbound.client.call('too-large')).rejects.toThrow('oversized')
    expect(outbound.socket.writes).toHaveLength(0)
    outbound.client.dispose()
  })

  it('connects, reports connection errors and timeouts, and validates addresses', async () => {
    const connectedSocket = new FakeSocket()
    net.createConnection.mockImplementationOnce(() => {
      queueMicrotask(() => connectedSocket.emit('connect'))
      return connectedSocket
    })
    const connected = await JsonRpcClient.connect('127.0.0.1:49978')
    expect(net.createConnection).toHaveBeenCalledWith({ host: '127.0.0.1', port: 49978 })
    connected.dispose()

    const failedSocket = new FakeSocket()
    const connectError = new Error('connection refused')
    net.createConnection.mockImplementationOnce(() => {
      queueMicrotask(() => failedSocket.emit('error', connectError))
      return failedSocket
    })
    await expect(JsonRpcClient.connect('localhost:49979')).rejects.toBe(connectError)

    vi.useFakeTimers()
    const timedOutSocket = new FakeSocket()
    net.createConnection.mockReturnValueOnce(timedOutSocket)
    const timedOut = JsonRpcClient.connect('localhost:49980', { connectTimeoutMs: 5 })
    const assertion = expect(timedOut).rejects.toThrow('Timed out connecting')
    await vi.advanceTimersByTimeAsync(5)
    await assertion
    expect(timedOutSocket.destroyReasons).toEqual([undefined])

    for (const address of [
      'localhost',
      ':9000',
      'localhost:0',
      'localhost:65536',
      'localhost:not-a-port',
      'localhost:1.5',
    ]) {
      await expect(JsonRpcClient.connect(address)).rejects.toThrow('Invalid Loopal Hub TCP address')
    }
  })
})
