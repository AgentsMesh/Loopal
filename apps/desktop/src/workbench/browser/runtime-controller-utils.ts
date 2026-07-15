export function errorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason)
}
