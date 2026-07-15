import { describe, expect, it, vi } from 'vitest'
import { timeout } from '../../../base/common/async'
import { CancellationTokenSource } from '../../../base/common/cancellation'
import { Emitter } from '../../../base/common/event'
import {
  createChannelConnection,
  type TestChannelContext,
} from '../../../../test/support/ipc/channel'
import { ChannelClientImpl, ChannelServer } from './channel'
import { MemoryTransport } from './transport'

describe('ChannelClient', () => {
  it('calls explicitly registered channel commands', async () => {
    const call = vi.fn(async (context: TestChannelContext, command: string, arg: unknown) => ({
      context,
      command,
      arg,
    }))
    const { client, server } = createChannelConnection({ call })
    await expect(client.call('test', 'echo', { value: 3 })).resolves.toEqual({
      context: { user: 'stone' },
      command: 'echo',
      arg: { value: 3 },
    })
    await expect(client.call('test', 'empty')).resolves.toEqual({
      context: { user: 'stone' },
      command: 'empty',
    })
    client.dispose()
    server.dispose()
  })

  it('cancels active calls from the client', async () => {
    const observed = vi.fn()
    const { client, server } = createChannelConnection({
      call: async (_context, _command, _arg, token) => {
        token.onCancellationRequested(observed)
        await timeout(100, token)
      },
    })
    const source = new CancellationTokenSource()
    const pending = client.call('test', 'slow', undefined, source.token)
    await Promise.resolve()
    source.cancel()
    await expect(pending).rejects.toThrow('cancelled')
    await Promise.resolve()
    expect(observed).toHaveBeenCalledOnce()
    client.dispose()
    server.dispose()
  })

  it('rejects calls cancelled before sending', async () => {
    const { client, server } = createChannelConnection({ call: vi.fn() })
    const source = new CancellationTokenSource()
    source.cancel()
    await expect(client.call('test', 'never', undefined, source.token)).rejects.toThrow('cancelled')
    client.dispose()
    server.dispose()
  })

  it('subscribes and unsubscribes remote events', async () => {
    const emitter = new Emitter<number>()
    const listen = vi.fn(() => emitter.event)
    const { client, server } = createChannelConnection({ call: vi.fn(), listen })
    const values: number[] = []
    const subscription = client.listen<number>('test', 'changed', { scope: 1 })((value) => {
      values.push(value)
    })
    await Promise.resolve()
    emitter.fire(4)
    await Promise.resolve()
    expect(values).toEqual([4])
    expect(listen).toHaveBeenCalledWith({ user: 'stone' }, 'changed', { scope: 1 })
    subscription.dispose()
    subscription.dispose()
    await Promise.resolve()
    emitter.fire(5)
    await Promise.resolve()
    expect(values).toEqual([4])
    client.dispose()
    server.dispose()
  })

  it('ignores malformed, duplicate, and late wire messages', async () => {
    const [clientTransport, serverTransport] = MemoryTransport.pair()
    const client = new ChannelClientImpl(clientTransport)
    const server = new ChannelServer(serverTransport, { user: 'stone' })
    server.registerChannel('test', { call: async () => 'ok' })
    serverTransport.send({ nope: true })
    clientTransport.send({ nope: true })
    serverTransport.send({ type: 'response', id: 999, ok: true, result: 'late' })
    serverTransport.send({ type: 'event', id: 999, data: 'late' })
    await Promise.resolve()
    await expect(client.call('test', 'run')).resolves.toBe('ok')
    client.dispose()
    server.dispose()
  })

  it('rejects pending calls when disposed', async () => {
    const { client, server } = createChannelConnection({
      call: async () => new Promise(() => undefined),
    })
    const pending = client.call('test', 'hang')
    await Promise.resolve()
    client.dispose()
    await expect(pending).rejects.toThrow('disposed')
    server.dispose()
  })
})
