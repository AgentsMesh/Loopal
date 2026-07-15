import { describe, expect, it } from 'vitest'
import {
  DESKTOP_CAPABILITY_HUB_UI,
  DESKTOP_CAPABILITY_WORKSPACE,
  DESKTOP_EVENT_PREFIX,
  DESKTOP_HANDSHAKE_PREFIX,
  DESKTOP_PROTOCOL_VERSION,
  DESKTOP_TRANSPORT,
  DesktopHandshakeSchema,
  DesktopSessionCreatedHandshakeSchema,
  parseDesktopHandshakeLine,
} from './desktop-handshake'

const common = {
  protocol_version: DESKTOP_PROTOCOL_VERSION,
  server_version: '0.6.3',
  pid: 1200,
  parent_pid: 1100,
}

function line(payload: unknown, ending = '\n'): string {
  return `${DESKTOP_HANDSHAKE_PREFIX}${JSON.stringify(payload)}${ending}`
}

describe('Desktop Host handshake', () => {
  it('parses alive, ready, and error records with LF or CRLF framing', () => {
    const alive = {
      ...common,
      phase: 'alive' as const,
      addr: '127.0.0.1:49978',
      token: 'secret-token',
      transport: DESKTOP_TRANSPORT,
      capabilities: [
        DESKTOP_CAPABILITY_HUB_UI,
        DESKTOP_CAPABILITY_WORKSPACE,
        'future_surface_v2',
      ],
    }
    const ready = {
      protocol_version: DESKTOP_PROTOCOL_VERSION,
      server_version: '0.6.3',
      pid: 1200,
      phase: 'ready' as const,
      session_id: 'session-1',
    }
    const error = {
      ...common,
      phase: 'error' as const,
      code: 'startup_failed',
      message: 'could not start',
    }

    expect(parseDesktopHandshakeLine(line(alive))).toEqual(alive)
    expect(parseDesktopHandshakeLine(line(ready, '\r\n'))).toEqual(ready)
    expect(parseDesktopHandshakeLine(line(error, '\n\n'))).toEqual(error)
    expect(DesktopHandshakeSchema.parse(alive)).toEqual(alive)
  })

  it('ignores ordinary stdout without attempting JSON parsing', () => {
    expect(parseDesktopHandshakeLine('Loopal is starting\n')).toBeUndefined()
    expect(parseDesktopHandshakeLine('')).toBeUndefined()
  })

  it('parses session-created only on its backward-compatible optional prefix', () => {
    const created = { ...common, phase: 'session_created' as const, session_id: 'session-1' }
    expect(parseDesktopHandshakeLine(
      `${DESKTOP_EVENT_PREFIX}${JSON.stringify(created)}\n`,
    )).toEqual(created)
    expect(DesktopSessionCreatedHandshakeSchema.parse(created)).toEqual(created)
    expect(() => parseDesktopHandshakeLine(line(created))).toThrow()
    expect(() => parseDesktopHandshakeLine(
      `${DESKTOP_EVENT_PREFIX}${JSON.stringify({
        ...common, phase: 'ready', session_id: 'session-1',
      })}\n`,
    )).toThrow()
  })

  it.each([
    ['malformed JSON', `${DESKTOP_HANDSHAKE_PREFIX}{not-json}`],
    ['wrong protocol', line({ ...common, protocol_version: 2, phase: 'ready', session_id: 's' })],
    ['non-loopback address', line({
      ...common,
      phase: 'alive',
      addr: '0.0.0.0:9000',
      token: 'token',
      transport: DESKTOP_TRANSPORT,
      capabilities: [DESKTOP_CAPABILITY_HUB_UI],
    })],
    ['missing capability', line({
      ...common,
      phase: 'alive',
      addr: '127.0.0.1:9000',
      token: 'token',
      transport: DESKTOP_TRANSPORT,
      capabilities: [],
    })],
    ['unknown field', line({ ...common, phase: 'ready', session_id: 's', extra: true })],
    ['empty error message', line({ ...common, phase: 'error', code: 'failed', message: '' })],
    ['invalid pid', line({ ...common, pid: 0, phase: 'ready', session_id: 's' })],
  ])('rejects %s', (_name, value) => {
    expect(() => parseDesktopHandshakeLine(value)).toThrow()
  })
})
