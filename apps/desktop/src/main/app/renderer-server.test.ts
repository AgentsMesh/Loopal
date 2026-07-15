import { beforeEach, describe, expect, it, vi } from 'vitest'
import { type DesktopBackend } from '../../platform/loopal-backend/common/backend'
import { Emitter } from '../../base/common/event'

const electron = vi.hoisted(() => {
  const listeners = new Map<string, Set<(event: unknown) => void>>()
  const ipcMain = {
    on: vi.fn((channel: string, listener: (event: unknown) => void) => {
      const values = listeners.get(channel) ?? new Set()
      values.add(listener)
      listeners.set(channel, values)
    }),
    removeListener: vi.fn((channel: string, listener: (event: unknown) => void) => {
      listeners.get(channel)?.delete(listener)
    }),
    emit(channel: string, event: unknown) {
      for (const listener of listeners.get(channel) ?? []) {
        listener(event)
      }
    },
    reset() {
      listeners.clear()
      ipcMain.on.mockClear()
      ipcMain.removeListener.mockClear()
    },
  }

  function port() {
    const messageListeners = new Set<(event: { data: unknown }) => void>()
    const closeListeners = new Set<() => void>()
    return {
      postMessage: vi.fn(),
      start: vi.fn(),
      close: vi.fn(),
      on: vi.fn((type: string, listener: (event: { data: unknown }) => void) => {
        if (type === 'message') messageListeners.add(listener)
      }),
      off: vi.fn((type: string, listener: (event: { data: unknown }) => void) => {
        if (type === 'message') messageListeners.delete(listener)
      }),
      once: vi.fn((type: string, listener: () => void) => {
        if (type === 'close') closeListeners.add(listener)
      }),
      emitMessage(data: unknown) {
        for (const listener of messageListeners) listener({ data })
      },
      emitClose() {
        for (const listener of closeListeners) listener()
        closeListeners.clear()
      },
    }
  }

  const channels: Array<{ port1: ReturnType<typeof port>; port2: ReturnType<typeof port> }> = []
  class MessageChannelMain {
    readonly port1 = port()
    readonly port2 = port()
    constructor() {
      channels.push({ port1: this.port1, port2: this.port2 })
    }
  }
  return { ipcMain, MessageChannelMain, channels }
})

vi.mock('electron', () => ({
  ipcMain: electron.ipcMain,
  MessageChannelMain: electron.MessageChannelMain,
}))

import {
  mainWindowPolicy,
  registerRendererServer,
} from './renderer-server'
import { createBackendStub } from '../../../test/support/backend/backend-stub'
import { RENDERER_CONNECT_CHANNEL } from '../../shared/protocol/renderer-protocol'
import { type DesktopEvent } from '../../shared/contracts'

function backend(): DesktopBackend {
  const emitter = new Emitter<DesktopEvent>()
  return createBackendStub({ onEvent: emitter.event })
}

describe('renderer channel server', () => {
  beforeEach(() => {
    electron.ipcMain.reset()
    electron.channels.length = 0
  })

  it('accepts only the owning main frame and serves the explicit backend channel', async () => {
    const frame = { url: 'file:///renderer/index.html', postMessage: vi.fn() }
    const webContents = { id: 7, mainFrame: frame }
    const policy = mainWindowPolicy(webContents as never)
    const service = backend()
    const registration = registerRendererServer(service, policy)
    const connect = electron.ipcMain.on.mock.calls.at(-1)?.[1]
    expect(connect).toBeTypeOf('function')

    const rejected = { sender: { id: 9 }, senderFrame: frame }
    connect!(rejected)
    expect(electron.channels).toHaveLength(0)

    const event = { sender: webContents, senderFrame: frame }
    connect!(event)
    expect(electron.channels).toHaveLength(1)
    expect(frame.postMessage).toHaveBeenCalledWith(
      'loopal-desktop:port',
      { protocolVersion: 2 },
      [electron.channels[0]?.port2],
    )

    const port = electron.channels[0]!.port1
    port.emitMessage({
      type: 'request',
      id: 1,
      channel: 'desktopBackend',
      command: 'bootstrap',
    })
    await Promise.resolve()
    await Promise.resolve()
    expect(port.postMessage).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'response', id: 1, ok: true }),
    )

    port.emitClose()
    registration.dispose()
    expect(electron.ipcMain.removeListener).toHaveBeenCalled()
  })

  it('requires both the expected web contents and its main frame', () => {
    const mainFrame = { url: 'file:///main', postMessage: vi.fn() }
    const webContents = { id: 1, mainFrame }
    const policy = mainWindowPolicy(webContents as never)
    expect(policy.isAllowed({ sender: webContents, senderFrame: mainFrame } as never)).toBe(true)
    expect(policy.isAllowed({ sender: webContents, senderFrame: {} } as never)).toBe(false)
    expect(policy.isAllowed({ sender: {}, senderFrame: mainFrame } as never)).toBe(false)
  })

  it('disposes all open renderer connections', () => {
    const registration = registerRendererServer(backend(), { isAllowed: () => true })
    const frame = { url: 'file:///renderer', postMessage: vi.fn() }
    const connect = electron.ipcMain.on.mock.calls.at(-1)?.[1]
    expect(connect).toBeTypeOf('function')
    connect!({
      sender: { id: 1 },
      senderFrame: frame,
    })
    expect(electron.channels).toHaveLength(1)
    registration.dispose()
    registration.dispose()
    expect(electron.channels[0]?.port1.close).toHaveBeenCalledOnce()
  })
})
