import {
  ChannelClientImpl,
  ChannelServer,
  type ServerChannel,
} from '../../../src/platform/ipc/common/channel'
import { MemoryTransport } from '../../../src/platform/ipc/common/transport'

export interface TestChannelContext {
  readonly user: string
}

export function createChannelConnection(channel: ServerChannel<TestChannelContext>) {
  const [clientTransport, serverTransport] = MemoryTransport.pair()
  const client = new ChannelClientImpl(clientTransport)
  const server = new ChannelServer(serverTransport, { user: 'stone' })
  server.registerChannel('test', channel)
  return { client, server, clientTransport, serverTransport }
}
