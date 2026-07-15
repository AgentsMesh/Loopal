import { type MetaHubRuntimeTarget } from './metahub-contracts'

export function federationHubName(
  base: string,
  target: MetaHubRuntimeTarget,
): string {
  const session = target.sessionId.replace(/[^a-zA-Z0-9_-]+/g, '-').replace(/^-+|-+$/g, '')
  const identity = `${session.slice(0, 24) || 'session'}-g${target.generation}-${hash(target.runtimeId)}`
  const prefix = base.slice(0, Math.max(1, 127 - identity.length))
  return `${prefix}-${identity}`
}

function hash(value: string): string {
  let result = 2166136261
  for (const character of value) {
    result ^= character.charCodeAt(0)
    result = Math.imul(result, 16777619)
  }
  return (result >>> 0).toString(36)
}
