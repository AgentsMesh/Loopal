import { DesktopEventSchema, type DesktopEvent } from '../../../../shared/contracts'

const eventTypes: Readonly<Record<string, DesktopEvent['type']>> = {
  'workspace/fileChanged': 'file_changed',
  'workspace/gitChanged': 'git_changed',
  'workspace/resyncRequired': 'workspace_resync_required',
}

export function projectCodeWorkbenchEvent(
  method: string,
  params: unknown,
): DesktopEvent | undefined {
  const type = eventTypes[method]
  if (!type || !isRecord(params)) return undefined
  const parsed = DesktopEventSchema.safeParse({ type, ...params })
  return parsed.success ? parsed.data : undefined
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
