import { describe, expect, it, vi } from 'vitest'
import { Emitter } from '../../../base/common/event'
import { createChannelConnection } from '../../../../test/support/ipc/channel'
import { ChannelClientImpl, ChannelServer, type ServerChannel } from './channel'
import { MemoryTransport } from './transport'
import { RemoteError } from './wire'

interface Context {
  readonly user: string
}

describe('ChannelServer', () => {
  it('returns structured errors for unknown channels and handlers', async () => {
    const { client, server } = createChannelConnection({
      call: async () => { throw new RemoteError('DENIED', 'not allowed', { scope: 'workspace' }) },
    })
    await expect(client.call('missing', 'run')).rejects.toMatchObject({
      code: 'CHANNEL_NOT_FOUND',
    })
    await expect(client.call('test', 'run')).rejects.toMatchObject({
      code: 'DENIED',
      data: { scope: 'workspace' },
    })
    client.dispose()
    server.dispose()
  })

  it('surfaces missing event handlers as event payloads', async () => {
    const { client, server } = createChannelConnection({ call: vi.fn() })
    const received = vi.fn()
    const subscription = client.listen('test', 'missing')(received)
    await Promise.resolve()
    await Promise.resolve()
    expect(received).toHaveBeenCalledWith({
      error: {
        code: 'EVENT_NOT_FOUND',
        message: 'Channel has no events: test',
      },
    })
    subscription.dispose()
    client.dispose()
    server.dispose()
  })

  it('enforces registration and disposal invariants', async () => {
    const [clientTransport, serverTransport] = MemoryTransport.pair()
    const server = new ChannelServer(serverTransport, { user: 'stone' })
    const channel: ServerChannel<Context> = { call: async () => 'ok' }
    const registration = server.registerChannel('test', channel)
    expect(() => server.registerChannel('test', channel)).toThrow('already registered')
    registration.dispose()

    const client = new ChannelClientImpl(clientTransport)
    await expect(client.call('test', 'run')).rejects.toMatchObject({ code: 'CHANNEL_NOT_FOUND' })
    client.dispose()
    client.dispose()
    await expect(client.call('test', 'late')).rejects.toThrow('disposed')
    client.listen('test', 'late')(() => undefined).dispose()
    server.dispose()
    server.dispose()
    expect(() => server.registerChannel('late', channel)).toThrow('disposed')
  })

  it('disposes active client and server subscriptions', async () => {
    let remoteListener: ((value: unknown) => void) | undefined
    const remoteDispose = vi.fn()
    const { client, server } = createChannelConnection({
      call: vi.fn(),
      listen: () => (listener) => {
        remoteListener = listener
        return { dispose: remoteDispose }
      },
    })
    const localListener = vi.fn()
    const subscription = client.listen('test', 'changed')(localListener)
    await Promise.resolve()

    client.dispose()
    server.dispose()
    remoteListener?.('late')
    subscription.dispose()

    expect(remoteDispose).toHaveBeenCalledOnce()
    expect(localListener).not.toHaveBeenCalled()
  })

  it('cancels active calls during disposal without sending a late response', async () => {
    let release: (() => void) | undefined
    const cancelled = vi.fn()
    const { client, server } = createChannelConnection({
      call: async (_context, _command, _arg, token) => {
        token.onCancellationRequested(cancelled)
        await new Promise<void>((resolve) => { release = resolve })
        return 'late'
      },
    })
    const pending = client.call('test', 'slow')
    await Promise.resolve()
    await Promise.resolve()

    server.dispose()
    expect(cancelled).toHaveBeenCalledOnce()
    release?.()
    await Promise.resolve()
    client.dispose()
    await expect(pending).rejects.toThrow('disposed')
  })

  it('ignores server-side no-op and opposite-direction messages', async () => {
    const [clientTransport, serverTransport] = MemoryTransport.pair()
    const server = new ChannelServer(serverTransport, { user: 'stone' })
    clientTransport.send({ type: 'cancel', id: 999 })
    clientTransport.send({ type: 'unsubscribe', id: 999 })
    clientTransport.send({ type: 'response', id: 999, ok: true, result: 'opposite' })
    clientTransport.send({ type: 'event', id: 999, data: 'opposite' })
    await Promise.resolve()
    await Promise.resolve()
    server.dispose()
    clientTransport.dispose()
  })
})
