import { z } from 'zod'

const requestId = z.number().int().positive()

export const RequestMessageSchema = z.object({
  type: z.literal('request'),
  id: requestId,
  channel: z.string().min(1),
  command: z.string().min(1),
  arg: z.unknown().optional(),
})

export const ResponseMessageSchema = z.discriminatedUnion('ok', [
  z.object({
    type: z.literal('response'),
    id: requestId,
    ok: z.literal(true),
    result: z.unknown().optional(),
  }),
  z.object({
    type: z.literal('response'),
    id: requestId,
    ok: z.literal(false),
    error: z.object({
      code: z.string(),
      message: z.string(),
      data: z.unknown().optional(),
    }),
  }),
])

export const CancelMessageSchema = z.object({
  type: z.literal('cancel'),
  id: requestId,
})

export const SubscribeMessageSchema = z.object({
  type: z.literal('subscribe'),
  id: requestId,
  channel: z.string().min(1),
  event: z.string().min(1),
  arg: z.unknown().optional(),
})

export const UnsubscribeMessageSchema = z.object({
  type: z.literal('unsubscribe'),
  id: requestId,
})

export const EventMessageSchema = z.object({
  type: z.literal('event'),
  id: requestId,
  data: z.unknown().optional(),
})

export const WireMessageSchema = z.discriminatedUnion('type', [
  RequestMessageSchema,
  ResponseMessageSchema,
  CancelMessageSchema,
  SubscribeMessageSchema,
  UnsubscribeMessageSchema,
  EventMessageSchema,
])

export type RequestMessage = z.infer<typeof RequestMessageSchema>
export type ResponseMessage = z.infer<typeof ResponseMessageSchema>
export type CancelMessage = z.infer<typeof CancelMessageSchema>
export type SubscribeMessage = z.infer<typeof SubscribeMessageSchema>
export type UnsubscribeMessage = z.infer<typeof UnsubscribeMessageSchema>
export type EventMessage = z.infer<typeof EventMessageSchema>
export type WireMessage = z.infer<typeof WireMessageSchema>

export interface SerializedError {
  readonly code: string
  readonly message: string
  readonly data?: unknown
}

export class RemoteError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly data?: unknown,
  ) {
    super(message)
    this.name = 'RemoteError'
  }
}

export function serializeError(error: unknown): SerializedError {
  if (error instanceof RemoteError) {
    const serialized: SerializedError = {
      code: error.code,
      message: error.message,
    }
    return error.data === undefined ? serialized : { ...serialized, data: error.data }
  }
  if (error instanceof Error) {
    return { code: error.name || 'ERROR', message: error.message }
  }
  return { code: 'ERROR', message: String(error) }
}
