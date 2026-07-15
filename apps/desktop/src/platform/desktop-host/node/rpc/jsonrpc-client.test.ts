import { afterEach, describe, expect, it, vi } from 'vitest'
import { FakeSocket, response } from '../../../../../test/support/ipc/jsonrpc'
import { JsonRpcClient } from './jsonrpc-client'

function createClient(options: ConstructorParameters<typeof JsonRpcClient>[1] = {}) {
  const socket = new FakeSocket()
  const client = new JsonRpcClient(socket as never, options)
  return { client, socket }
}

afterEach(() => vi.useRealTimers())

describe('JsonRpcClient', () => {
  it('correlates fragmented and coalesced responses using default options', async () => {
    const { client, socket } = createClient()
    const first = client.call('hub/first')
    expect(JSON.parse(socket.writes[0]!.trim())).toEqual({
      jsonrpc: '2.0',
      id: 1,
      method: 'hub/first',
      params: {},
    })
    const encoded = response(1, { ok: true })
    socket.data(encoded.slice(0, 12))
    socket.data(`${encoded.slice(12).trimEnd()}\r\n\n`)
    await expect(first).resolves.toEqual({ ok: true })

    const second = client.call('hub/second', { value: 2 })
    const third = client.call('hub/third', null)
    socket.data(`${response(3, 'third')}${response(2, 'second')}`)
    await expect(second).resolves.toBe('second')
    await expect(third).resolves.toBe('third')
    expect(socket.setEncoding).toHaveBeenCalledWith('utf8')

    const closed = vi.fn()
    client.onClose(closed)
    client.dispose()
    client.dispose()
    expect(socket.end).toHaveBeenCalledOnce()
    expect(socket.destroyReasons).toEqual([undefined])
    expect(closed).not.toHaveBeenCalled()
  })

  it('delivers notifications and rejects inbound Hub requests', () => {
    const { client, socket } = createClient()
    const notifications = vi.fn()
    client.onNotification(notifications)
    socket.data(
      `${JSON.stringify({ jsonrpc: '2.0', method: 'agent/event', params: { value: 1 } })}\n`,
    )
    socket.data(`${JSON.stringify({ jsonrpc: '2.0', method: 'without/params' })}\n`)
    socket.data(`${JSON.stringify({ jsonrpc: '2.0', id: 'notification-id', method: 'string/id' })}\n`)
    socket.data(`${JSON.stringify({ jsonrpc: '2.0', id: 41, method: 'hub/calls/desktop' })}\n`)

    expect(notifications).toHaveBeenNthCalledWith(1, {
      method: 'agent/event',
      params: { value: 1 },
    })
    expect(notifications).toHaveBeenNthCalledWith(2, {
      method: 'without/params',
      params: undefined,
    })
    expect(notifications).toHaveBeenNthCalledWith(3, {
      method: 'string/id',
      params: undefined,
    })
    expect(JSON.parse(socket.writes.at(-1)!.trim())).toEqual({
      jsonrpc: '2.0',
      id: 41,
      error: {
        code: -32601,
        message: 'Desktop does not expose Hub-callable methods',
      },
    })
    client.dispose()
  })

  it('returns typed remote errors, protocol defaults, and ignores late responses', async () => {
    const { client, socket } = createClient()
    const detailed = client.call('hub/detailed-error')
    socket.data(
      `${JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        error: { code: -32001, message: 'denied', data: { scope: 'workspace' } },
      })}\n`,
    )
    await expect(detailed).rejects.toMatchObject({
      name: 'JsonRpcRemoteError',
      code: -32001,
      message: 'denied',
      data: { scope: 'workspace' },
    })

    const fallback = client.call('hub/fallback-error')
    socket.data(`${JSON.stringify({ jsonrpc: '2.0', id: 2, error: { code: 'bad' } })}\n`)
    await expect(fallback).rejects.toEqual(
      expect.objectContaining({
        name: 'JsonRpcRemoteError',
        code: -32603,
        message: 'Remote error',
      }),
    )
    socket.data(response(2, 'late'))
    socket.data(response(999, 'unknown'))
    client.dispose()
  })

  it('supports preflight abort, active abort, timeout, and write failure', async () => {
    const { client, socket } = createClient({ requestTimeoutMs: 5 })
    const preflight = new AbortController()
    const preflightReason = new Error('already aborted')
    preflight.abort(preflightReason)
    await expect(client.call('never/sent', {}, preflight.signal)).rejects.toBe(preflightReason)

    const active = new AbortController()
    const pending = client.call('abort/active', {}, active.signal)
    const activeReason = new Error('stop now')
    active.abort(activeReason)
    await expect(pending).rejects.toBe(activeReason)

    let abortListener: (() => void) | undefined
    const removeAbortListener = vi.fn()
    const signal = {
      aborted: false,
      reason: undefined,
      addEventListener: (_type: string, listener: () => void) => { abortListener = listener },
      removeEventListener: removeAbortListener,
    } as unknown as AbortSignal
    const fallbackAbort = client.call('abort/fallback', {}, signal)
    abortListener?.()
    await expect(fallbackAbort).rejects.toThrow('request aborted')
    expect(removeAbortListener).toHaveBeenCalledOnce()

    vi.useFakeTimers()
    const timedOut = client.call('timeout/request')
    const assertion = expect(timedOut).rejects.toThrow('request timed out: timeout/request')
    await vi.advanceTimersByTimeAsync(5)
    await assertion

    socket.throwOnWrite = new Error('write failed')
    await expect(client.call('write/failure')).rejects.toThrow('write failed')
    client.dispose()
  })

  it('rejects pending work on error, clean close, and disposal', async () => {
    const errored = createClient()
    const errorClose = vi.fn()
    errored.client.onClose(errorClose)
    const errorPending = errored.client.call('pending/error')
    const socketError = new Error('socket failed')
    errored.socket.fail(socketError)
    await expect(errorPending).rejects.toBe(socketError)
    expect(errorClose).toHaveBeenCalledWith(socketError)
    errored.socket.close()
    await expect(errored.client.call('after/error')).rejects.toThrow('connection is closed')
    errored.client.dispose()

    const clean = createClient()
    const cleanClose = vi.fn()
    clean.client.onClose(cleanClose)
    const cleanPending = clean.client.call('pending/clean-close')
    clean.socket.close()
    await expect(cleanPending).rejects.toThrow('connection closed')
    expect(cleanClose).toHaveBeenCalledWith(undefined)
    clean.client.dispose()

    const disposed = createClient()
    const disposedPending = disposed.client.call('pending/dispose')
    disposed.client.dispose()
    await expect(disposedPending).rejects.toThrow('client disposed')
  })
})
