import { z } from 'zod'

export const DESKTOP_HANDSHAKE_PREFIX = 'LOOPAL_DESKTOP ' as const
export const DESKTOP_EVENT_PREFIX = 'LOOPAL_DESKTOP_EVENT ' as const
export const DESKTOP_PROTOCOL_VERSION = 1 as const
export const DESKTOP_TRANSPORT = 'tcp_jsonrpc_ndjson' as const
export const DESKTOP_CAPABILITY_HUB_UI = 'hub_ui_v1' as const
export const DESKTOP_CAPABILITY_WORKSPACE = 'workspace_v1' as const
export const DESKTOP_REQUIRED_CAPABILITIES = [
  DESKTOP_CAPABILITY_HUB_UI,
  DESKTOP_CAPABILITY_WORKSPACE,
] as const

const capabilities = z.array(z.string().min(1)).refine(
  (items) => DESKTOP_REQUIRED_CAPABILITIES.every((item) => items.includes(item)),
  'required desktop capabilities are missing',
)

const common = z.object({
  protocol_version: z.literal(DESKTOP_PROTOCOL_VERSION),
  server_version: z.string().min(1),
  pid: z.number().int().positive(),
  parent_pid: z.number().int().positive().optional(),
})

export const DesktopHandshakeSchema = z.discriminatedUnion('phase', [
  common.extend({
    phase: z.literal('alive'),
    addr: z.string().regex(/^127\.0\.0\.1:\d+$/),
    token: z.string().min(1),
    transport: z.literal(DESKTOP_TRANSPORT),
    capabilities,
  }).strict(),
  common.extend({
    phase: z.literal('ready'),
    session_id: z.string().min(1),
  }).strict(),
  common.extend({
    phase: z.literal('error'),
    code: z.string().min(1),
    message: z.string().min(1),
  }).strict(),
])

export const DesktopSessionCreatedHandshakeSchema = common.extend({
  phase: z.literal('session_created'),
  session_id: z.string().min(1),
}).strict()

export type DesktopCoreHandshake = z.infer<typeof DesktopHandshakeSchema>
export type DesktopSessionCreatedHandshake = z.infer<
  typeof DesktopSessionCreatedHandshakeSchema
>
export type DesktopHandshake = DesktopCoreHandshake | DesktopSessionCreatedHandshake
export type DesktopAliveHandshake = Extract<DesktopCoreHandshake, { phase: 'alive' }>
export type DesktopReadyHandshake = Extract<DesktopCoreHandshake, { phase: 'ready' }>
export type DesktopActivationHandshake = DesktopSessionCreatedHandshake | DesktopReadyHandshake

/** Parse one physical stdout line. Non-protocol logging is deliberately ignored. */
export function parseDesktopHandshakeLine(line: string): DesktopHandshake | undefined {
  const normalized = line.replace(/[\r\n]+$/, '')
  if (normalized.startsWith(DESKTOP_EVENT_PREFIX)) {
    return DesktopSessionCreatedHandshakeSchema.parse(
      JSON.parse(normalized.slice(DESKTOP_EVENT_PREFIX.length)),
    )
  }
  if (!normalized.startsWith(DESKTOP_HANDSHAKE_PREFIX)) return undefined
  return DesktopHandshakeSchema.parse(
    JSON.parse(normalized.slice(DESKTOP_HANDSHAKE_PREFIX.length)),
  )
}
