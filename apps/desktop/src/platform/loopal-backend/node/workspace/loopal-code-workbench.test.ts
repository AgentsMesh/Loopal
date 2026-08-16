import { describe, expect, it, vi } from 'vitest'
import { CancellationToken, CancellationTokenSource } from '../../../../base/common/cancellation'
import { type DesktopHostClient } from '../backend/loopal-backend-types'
import { bindCodeWorkbench } from './loopal-code-workbench-bind'
import {
  LoopalCodeWorkbench,
  type CodeWorkbenchRuntimeRouter,
} from './loopal-code-workbench'
import { type SessionRuntimeHandle } from '../runtime/session-runtime-registry'

function response(method: string, params: unknown): unknown {
  const input = params as Record<string, unknown>
  if (method === 'workspace/listDirectory') {
    return { workspaceId: input.workspaceId, path: input.path, entries: [] }
  }
  if (method === 'workspace/readFile' || method === 'workspace/writeFile') {
    return {
      workspaceId: input.workspaceId, path: input.path, content: input.content ?? 'text',
      version: 'v1', languageId: 'rust', readonly: false,
    }
  }
  if (method === 'workspace/search') return { matches: [], truncated: false }
  if (method === 'workspace/gitStatus') return { branch: 'main', ahead: 0, behind: 0, changes: [] }
  if (method === 'workspace/gitDiff') return { path: input.path, patch: '', original: '', modified: '' }
  if (method === 'workspace/listWorktrees') return []
  if (method === 'workspace/createWorktree') {
    return { id: input.name, path: '/tmp/wt', branch: 'feature', head: 'abc', isMain: false, hasChanges: false }
  }
  return { ok: true }
}

function harness() {
  const request = vi.fn<DesktopHostClient['request']>(async (method, params) => response(method, params))
  const host = { request } as unknown as DesktopHostClient
  const runtime: SessionRuntimeHandle = {
    workspaceId: 'workspace', sessionId: 'session', runtimeId: 'runtime-1',
    generation: 1, host,
  }
  let live: SessionRuntimeHandle | undefined = runtime
  const router: CodeWorkbenchRuntimeRouter = {
    workspace: vi.fn(async () => runtime),
    liveSession: vi.fn(async () => live),
  }
  const setLive = (value?: SessionRuntimeHandle): void => {
    live = value
  }
  return { code: new LoopalCodeWorkbench(router), request, router, runtime, setLive }
}

const intentDigest = `sha256:${'ab'.repeat(32)}`

describe('LoopalCodeWorkbench routing', () => {
  it('maps workspace operations and binds their schemas', async () => {
    const { code, request } = harness()
    const token = CancellationToken.None
    await expect(code.listDirectory({ workspaceId: 'workspace', path: '' }, token))
      .resolves.toMatchObject({ workspaceId: 'workspace' })
    await expect(code.readFile({ workspaceId: 'workspace', path: 'a.rs' }, token))
      .resolves.toMatchObject({ languageId: 'rust' })
    await code.writeFile({
      workspaceId: 'workspace', path: 'a.rs', content: 'next', expectedVersion: 'v0',
    }, token)
    await code.searchWorkspace({ workspaceId: 'workspace', query: 'next' }, token)
    await code.gitStatus('workspace', token)
    await code.gitDiff({ workspaceId: 'workspace', path: 'a.rs' }, token)
    await code.gitStage({ workspaceId: 'workspace', path: 'a.rs' }, token)
    await code.gitUnstage({ workspaceId: 'workspace', path: 'a.rs' }, token)
    await code.listWorktrees('workspace', token)
    await code.createWorktree({ workspaceId: 'workspace', name: 'feature' }, token)
    await code.removeWorktree({ workspaceId: 'workspace', name: 'feature', force: false }, token)
    expect(request.mock.calls.map(([method]) => method)).toEqual([
      'workspace/listDirectory', 'workspace/readFile', 'workspace/writeFile', 'workspace/search',
      'workspace/gitStatus', 'workspace/gitDiff', 'workspace/gitStage', 'workspace/gitUnstage',
      'workspace/listWorktrees', 'workspace/createWorktree', 'workspace/removeWorktree',
    ])

    request.mockResolvedValueOnce({ invalid: true })
    await expect(bindCodeWorkbench(code).readFile({
      workspaceId: 'workspace', path: 'bad.rs',
    }, token)).rejects.toThrow()
  })

  it('responds only through the exact live generation', async () => {
    const { code, request, setLive } = harness()
    const permission = {
      sessionId: 'session', runtimeId: 'runtime-1', generation: 1,
      agentId: 'worker', requestId: 'permission', intentDigest,
      decision: 'allow_once' as const,
    }
    await code.respondPermission(permission, CancellationToken.None)
    expect(request).toHaveBeenLastCalledWith('hub/permission_response', {
      agent_name: 'worker', tool_call_id: 'permission',
      permission_intent_digest: intentDigest, allow: true,
    }, expect.any(AbortSignal))
    await code.respondPermission({ ...permission, requestId: 'deny', decision: 'deny' }, CancellationToken.None)
    expect(request).toHaveBeenLastCalledWith('hub/permission_response', {
      agent_name: 'worker', tool_call_id: 'deny',
      permission_intent_digest: intentDigest, allow: false,
    }, expect.any(AbortSignal))
    const question = {
      sessionId: 'session', runtimeId: 'runtime-1', generation: 1,
      agentId: 'worker', requestId: 'question',
    }
    await code.respondQuestion({ ...question, answers: ['yes'] }, CancellationToken.None)
    expect(request).toHaveBeenLastCalledWith('hub/question_response', {
      agent_name: 'worker', question_id: 'question',
      response: { kind: 'answered', question_id: 'question', answers: ['yes'] },
    }, expect.any(AbortSignal))
    await code.respondQuestion(
      { ...question, requestId: 'cancelled', cancelled: true }, CancellationToken.None,
    )
    expect(request).toHaveBeenLastCalledWith('hub/question_response', {
      agent_name: 'worker', question_id: 'cancelled',
      response: { kind: 'cancelled', question_id: 'cancelled' },
    }, expect.any(AbortSignal))
    await expect(code.respondPermission({ ...permission, generation: 2 }, CancellationToken.None))
      .rejects.toMatchObject({ code: 'RUNTIME_GONE' })
    setLive()
    await expect(code.respondQuestion({ ...question, answers: ['yes'] }, CancellationToken.None))
      .rejects.toMatchObject({ code: 'RUNTIME_GONE' })
  })

  it('checks cancellation before and after router resolution and during RPC', async () => {
    const { code, router, request, runtime } = harness()
    await expect(code.gitStatus('workspace', CancellationToken.Cancelled))
      .rejects.toThrow('cancelled')
    expect(router.workspace).not.toHaveBeenCalled()

    let release!: (runtime: SessionRuntimeHandle) => void
    router.workspace = vi.fn(() => new Promise<SessionRuntimeHandle>((resolve) => {
      release = resolve
    }))
    const source = new CancellationTokenSource()
    const pending = code.gitStatus('workspace', source.token)
    source.cancel()
    release(runtime)
    await expect(pending).rejects.toThrow('cancelled')
    expect(request).not.toHaveBeenCalled()

    router.workspace = vi.fn(async () => runtime)
    request.mockImplementation(() => new Promise((_resolve, reject) => {
      const signal = request.mock.calls.at(-1)?.[2]
      signal?.addEventListener('abort', () => reject(new Error('aborted')))
    }))
    const during = new CancellationTokenSource()
    const inFlight = code.gitStatus('workspace', during.token)
    await vi.waitFor(() => expect(request).toHaveBeenCalled())
    during.cancel()
    await expect(inFlight).rejects.toThrow('aborted')
  })
})
