import { type AgentControlCommand } from '../../../../shared/contracts'
import { type MessageKey } from '../../../../shared/i18n'

export type SlashArgumentMode = 'none' | 'optional' | 'required'
export type SlashCommandSource = 'runtime' | 'skill'

export interface SlashCommandItem {
  readonly name: string
  readonly description: string
  readonly usage: string
  readonly arguments: SlashArgumentMode
  readonly source: SlashCommandSource
  readonly sourceLabel: string
}

interface BuiltinSpec {
  readonly name: string
  readonly descriptionKey: MessageKey
  readonly usage: string
  readonly arguments: SlashArgumentMode
}

export const BUILTIN_SLASH_COMMANDS: readonly BuiltinSpec[] = [
  { name: '/act', descriptionKey: 'slash.command.act', usage: '/act', arguments: 'none' },
  { name: '/plan', descriptionKey: 'slash.command.plan', usage: '/plan', arguments: 'none' },
  { name: '/clear', descriptionKey: 'slash.command.clear', usage: '/clear', arguments: 'none' },
  { name: '/compact', descriptionKey: 'slash.command.compact', usage: '/compact [instructions]', arguments: 'optional' },
  { name: '/model', descriptionKey: 'slash.command.model', usage: '/model <model>', arguments: 'required' },
  { name: '/rewind', descriptionKey: 'slash.command.rewind', usage: '/rewind <turn>', arguments: 'required' },
  { name: '/permission', descriptionKey: 'slash.command.permission', usage: '/permission <mode>', arguments: 'required' },
  { name: '/decision', descriptionKey: 'slash.command.decision', usage: '/decision <mode>', arguments: 'required' },
  { name: '/sandbox', descriptionKey: 'slash.command.sandbox', usage: '/sandbox <policy>', arguments: 'required' },
  { name: '/suspend', descriptionKey: 'slash.command.suspend', usage: '/suspend', arguments: 'none' },
  { name: '/unsuspend', descriptionKey: 'slash.command.unsuspend', usage: '/unsuspend', arguments: 'none' },
  { name: '/mcp', descriptionKey: 'slash.command.mcp', usage: '/mcp [status|reconnect <server>|disconnect <server>]', arguments: 'optional' },
  { name: '/help', descriptionKey: 'slash.command.help', usage: '/help [command]', arguments: 'optional' },
]

export const BUILTIN_SLASH_NAMES = new Set(BUILTIN_SLASH_COMMANDS.map(({ name }) => name))

export type SlashErrorCode =
  | 'unexpected_arguments' | 'required_argument' | 'invalid_value' | 'value_too_long'

export type SlashParseResult =
  | { readonly kind: 'message' }
  | { readonly kind: 'help'; readonly query: string }
  | { readonly kind: 'control'; readonly command: AgentControlCommand }
  | { readonly kind: 'error'; readonly code: SlashErrorCode; readonly command: string; readonly usage: string }

export function parseSlashInput(input: string): SlashParseResult {
  const text = input.trim()
  if (!text.startsWith('/') || text.startsWith('//')) return { kind: 'message' }
  const boundary = text.search(/\s/)
  const name = boundary < 0 ? text : text.slice(0, boundary)
  const argument = boundary < 0 ? '' : text.slice(boundary).trim()
  const spec = BUILTIN_SLASH_COMMANDS.find((candidate) => candidate.name === name)
  if (!spec) return { kind: 'message' }
  if (spec.arguments === 'none' && argument) return commandError('unexpected_arguments', spec)
  if (spec.arguments === 'required' && !argument) return commandError('required_argument', spec)
  if (argument.length > 4_096) return commandError('value_too_long', spec)
  if (name === '/help') return { kind: 'help', query: argument.replace(/^\//, '') }
  if (name === '/act' || name === '/plan') {
    return { kind: 'control', command: { type: 'mode', mode: name.slice(1) as 'act' | 'plan' } }
  }
  if (name === '/clear' || name === '/suspend' || name === '/unsuspend') {
    return { kind: 'control', command: { type: name.slice(1) as 'clear' | 'suspend' | 'unsuspend' } }
  }
  if (name === '/compact') {
    return { kind: 'control', command: argument
      ? { type: 'compact', instructions: argument } : { type: 'compact' } }
  }
  if (name === '/model') return { kind: 'control', command: { type: 'model', model: argument } }
  if (name === '/rewind') {
    return /^\d+$/.test(argument) && Number.isSafeInteger(Number(argument))
      ? { kind: 'control', command: { type: 'rewind', turnIndex: Number(argument) } }
      : commandError('invalid_value', spec)
  }
  if (name === '/permission') return enumCommand(
    argument, ['bypass', 'ask_dangerous', 'ask_any_write'], spec,
    (mode) => ({ type: 'permission', mode }),
  )
  if (name === '/decision') return enumCommand(
    argument, ['manual', 'classifier', 'agent'], spec,
    (mode) => ({ type: 'decision', mode }),
  )
  if (name === '/sandbox') return enumCommand(
    argument, ['disabled', 'default_write', 'read_only'], spec,
    (policy) => ({ type: 'sandbox', policy }),
  )
  return parseMcp(argument, spec)
}

function parseMcp(argument: string, spec: BuiltinSpec): SlashParseResult {
  if (!argument || argument === 'status') return { kind: 'control', command: { type: 'mcp_status' } }
  const match = /^(reconnect|disconnect)\s+(\S+)$/.exec(argument)
  const action = match?.[1]
  const server = match?.[2]
  if (!action || !server || server.length > 512) return commandError('invalid_value', spec)
  return { kind: 'control', command: action === 'reconnect'
    ? { type: 'mcp_reconnect', server }
    : { type: 'mcp_disconnect', server } }
}

function enumCommand<T extends string>(
  value: string, allowed: readonly T[], spec: BuiltinSpec,
  create: (value: T) => AgentControlCommand,
): SlashParseResult {
  return allowed.includes(value as T)
    ? { kind: 'control', command: create(value as T) }
    : commandError('invalid_value', spec)
}

function commandError(code: SlashErrorCode, spec: BuiltinSpec): SlashParseResult {
  return { kind: 'error', code, command: spec.name, usage: spec.usage }
}
