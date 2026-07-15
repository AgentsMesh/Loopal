export function requirePositive(value: number, name: string): void {
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new Error(`Session runtime ${name} must be a positive integer`)
  }
}

export function requireText(value: string, name: string): void {
  if (!value.trim()) throw new Error(`Session runtime ${name} is required`)
}
