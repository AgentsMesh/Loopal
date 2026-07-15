import { describe, expect, it } from 'vitest'
import {
  RemoteError,
  WireMessageSchema,
  serializeError,
} from './wire'

describe('IPC wire protocol', () => {
  it('validates every message shape', () => {
    const messages = [
      { type: 'request', id: 1, channel: 'test', command: 'run', arg: { ok: true } },
      { type: 'response', id: 1, ok: true, result: 4 },
      { type: 'response', id: 1, ok: false, error: { code: 'NO', message: 'failed' } },
      { type: 'cancel', id: 1 },
      { type: 'subscribe', id: 1, channel: 'test', event: 'changed' },
      { type: 'unsubscribe', id: 1 },
      { type: 'event', id: 1, data: 'value' },
    ]
    for (const message of messages) {
      expect(WireMessageSchema.parse(message)).toEqual(message)
    }
    expect(WireMessageSchema.safeParse({ type: 'request', id: 0 }).success).toBe(false)
    expect(WireMessageSchema.safeParse({ type: 'unknown' }).success).toBe(false)
  })

  it('serializes typed, native, and non-error failures', () => {
    expect(serializeError(new RemoteError('DENIED', 'no', { path: '/x' }))).toEqual({
      code: 'DENIED',
      message: 'no',
      data: { path: '/x' },
    })
    expect(serializeError(new RemoteError('EMPTY', 'empty'))).toEqual({
      code: 'EMPTY',
      message: 'empty',
    })
    expect(serializeError(new TypeError('bad'))).toEqual({ code: 'TypeError', message: 'bad' })
    const unnamed = new Error('unnamed')
    unnamed.name = ''
    expect(serializeError(unnamed)).toEqual({ code: 'ERROR', message: 'unnamed' })
    expect(serializeError('failure')).toEqual({ code: 'ERROR', message: 'failure' })
    const error = new RemoteError('X', 'message', 2)
    expect(error.name).toBe('RemoteError')
    expect(error.data).toBe(2)
  })
})
