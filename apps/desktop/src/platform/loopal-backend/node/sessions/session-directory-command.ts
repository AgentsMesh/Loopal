import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import { z } from 'zod'
import { type SessionDirectoryRequest } from './session-directory-authority'

const execute = promisify(execFile)
const InspectParamsSchema = z.object({ path: z.string().min(1) }).strict()
const PrepareParamsSchema = z.object({
  path: z.string().min(1), name: z.string().min(1), expectedRoot: z.string().min(1),
  expectedHead: z.string().min(1),
}).strict()
const CleanupParamsSchema = z.object({
  path: z.string().min(1), name: z.string().min(1), expectedPath: z.string().min(1),
}).strict()
const EnvelopeSchema = z.union([
  z.object({ ok: z.literal(true), value: z.unknown() }).strict(),
  z.object({
    ok: z.literal(false),
    error: z.object({ code: z.string().min(1), message: z.string() }).strict(),
  }).strict(),
])

interface CommandOptions {
  readonly env: NodeJS.ProcessEnv
  readonly encoding: 'utf8'
  readonly maxBuffer: number
  readonly timeout: number
  readonly windowsHide: true
}
export type SessionDirectoryCommandRunner = (
  file: string, args: readonly string[], options: CommandOptions,
) => Promise<{ stdout: string }>

const runCommand: SessionDirectoryCommandRunner = async (file, args, options) => {
  const result = await execute(file, [...args], options)
  return { stdout: result.stdout }
}

export function createSessionDirectoryCommand(
  binaryPath: string,
  runner: SessionDirectoryCommandRunner = runCommand,
): SessionDirectoryRequest {
  return async (method, params) => {
    const args = commandArgs(method, params)
    const { stdout } = await runner(binaryPath, args, {
      env: safeEnvironment(process.env),
      encoding: 'utf8',
      maxBuffer: 1024 * 1024,
      timeout: 30_000,
      windowsHide: true,
    })
    const envelope = EnvelopeSchema.parse(JSON.parse(stdout.trim()))
    if (!envelope.ok) throw new Error(`${envelope.error.code}: ${envelope.error.message}`)
    return envelope.value
  }
}

function commandArgs(method: string, params: unknown): string[] {
  if (method === 'desktop/inspectWorkingDirectory') {
    const input = InspectParamsSchema.parse(params)
    return ['desktop', 'inspect-directory', '--path', input.path]
  }
  if (method === 'desktop/prepareWorktree') {
    const input = PrepareParamsSchema.parse(params)
    return [
      'desktop', 'prepare-worktree', '--path', input.path, '--name', input.name,
      '--expected-root', input.expectedRoot, '--expected-head', input.expectedHead,
    ]
  }
  if (method === 'desktop/cleanupWorktree') {
    const input = CleanupParamsSchema.parse(params)
    return [
      'desktop', 'cleanup-worktree', '--path', input.path, '--name', input.name,
      '--expected-path', input.expectedPath,
    ]
  }
  throw new Error(`Unsupported session-directory command: ${method}`)
}

function safeEnvironment(env: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const result: NodeJS.ProcessEnv = { LOOPAL_OTEL_ENABLED: 'false' }
  const exact = new Set([
    'PATH', 'HOME', 'TMPDIR', 'TMP', 'TEMP', 'SystemRoot', 'WINDIR', 'USERPROFILE',
    'GIT_CONFIG_NOSYSTEM', 'GIT_CONFIG_GLOBAL',
    'GIT_AUTHOR_NAME', 'GIT_AUTHOR_EMAIL', 'GIT_AUTHOR_DATE',
    'GIT_COMMITTER_NAME', 'GIT_COMMITTER_EMAIL', 'GIT_COMMITTER_DATE',
  ])
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined && exact.has(key)) result[key] = value
  }
  return result
}
