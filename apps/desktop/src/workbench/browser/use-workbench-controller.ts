import { useEffect, useMemo, useRef, useState } from 'react'
import {
  type CreateSessionInput, type DesktopEvent, type LoopalDesktopAPI,
  type SessionDetail, type Workspace,
} from '../../shared/contracts'
import { isSessionLive } from '../../shared/contracts/session-lifecycle'
import { createWorkbenchRegistries } from '../workbench-registries'
import {
  initialDesktopProjection, projectDesktopEvent, type DesktopProjection,
} from './desktop-event-projector'
import { preferredSessionId, sessionCatalogModel } from '../contrib/sessions/browser/session-catalog-model'
import { useImageAttachments } from '../contrib/conversation/browser/use-image-attachments'
import { useTargetDrafts } from '../contrib/conversation/browser/use-target-drafts'
export function useWorkbenchController(api: LoopalDesktopAPI) {
  const registries = useMemo(createWorkbenchRegistries, [])
  const [projection, setProjection] = useState<DesktopProjection>(initialDesktopProjection)
  const [activeSessionId, setActiveSessionId] = useState<string>()
  const [workspaces, setWorkspaces] = useState<readonly Workspace[]>([])
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string>()
  const [query, setQuery] = useState('')
  const [sending, setSending] = useState(false)
  const [lifecycleBusy, setLifecycleBusy] = useState(false)
  const [error, setError] = useState<string>()
  const searchRef = useRef<HTMLInputElement>(null)
  const selectionVersion = useRef(0)
  const imageAttachments = useImageAttachments(api, setError)
  const targetDrafts = useTargetDrafts(activeSessionId)
  useEffect(() => {
    let disposed = false
    let live = false
    const buffered: DesktopEvent[] = []
    const unsubscribe = api.onEvent((event) => {
      if (disposed) return
      if (!live) buffered.push(event)
      else setProjection((current) => projectDesktopEvent(current, event))
    })
    void api.bootstrap().then(async (bootstrap) => {
      if (disposed) return
      const base = buffered.reduce(projectDesktopEvent, {
        hostStatus: bootstrap.hostStatus,
        sessions: bootstrap.sessions,
        runtimes: bootstrap.runtimes,
      })
      buffered.length = 0
      live = true
      setProjection(base)
      setWorkspaces(bootstrap.workspaces)
      const selected = preferredSessionId(bootstrap.sessions, bootstrap.activeSessionId)
      const session = bootstrap.sessions.find((candidate) => candidate.id === selected)
      setActiveWorkspaceId(session?.workspaceId)
      setActiveSessionId(selected)
      if (selected) {
        const version = ++selectionVersion.current
        const detail = await api.openSession(selected)
        if (!disposed && version === selectionVersion.current) {
          setProjection((current) => ({ ...current, detail }))
        }
      }
    }).catch((reason: unknown) => {
      if (!disposed) setError(errorMessage(reason))
    })
    return () => {
      disposed = true
      selectionVersion.current++
      unsubscribe()
      registries.contributions.dispose()
    }
  }, [api, registries])
  useEffect(() => {
    const listener = (event: KeyboardEvent): void => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        searchRef.current?.focus()
      }
    }
    window.addEventListener('keydown', listener)
    return () => window.removeEventListener('keydown', listener)
  }, [])
  const catalog = useMemo(
    () => sessionCatalogModel(projection.sessions, query),
    [projection.sessions, query],
  )
  const selectDetail = (sessionId: string, detail: SessionDetail): void => {
    setActiveSessionId(sessionId)
    setActiveWorkspaceId(detail.session.workspaceId)
    setProjection((current) => ({
      ...current,
      sessions: current.sessions.some((session) => session.id === sessionId)
        ? current.sessions.map((session) => session.id === sessionId ? detail.session : session)
        : [...current.sessions, detail.session],
      detail,
    }))
  }
  const openSession = async (sessionId: string): Promise<SessionDetail | undefined> => {
    const version = ++selectionVersion.current
    const session = projection.sessions.find((candidate) => candidate.id === sessionId)
    setActiveSessionId(sessionId)
    if (session) setActiveWorkspaceId(session.workspaceId)
    setProjection(withoutDetail)
    setError(undefined)
    try {
      const detail = await api.openSession(sessionId)
      if (version !== selectionVersion.current) return
      selectDetail(sessionId, detail)
      return detail
    } catch (reason) {
      if (version === selectionVersion.current) setError(errorMessage(reason))
    }
  }
  const createSession = async (input: CreateSessionInput): Promise<string | undefined> => {
    if (lifecycleBusy) return 'SESSION_CREATE_UNAVAILABLE'
    const version = ++selectionVersion.current
    setLifecycleBusy(true)
    setError(undefined)
    try {
      const detail = await api.createSession(input)
      if (version === selectionVersion.current) selectDetail(detail.session.id, detail)
      try {
        const refreshed = await api.bootstrap()
        setWorkspaces(refreshed.workspaces)
      } catch (reason) {
        setError(`SESSION_WORKSPACE_REFRESH_FAILED: ${errorMessage(reason)}`)
      }
    } catch (reason) {
      const message = errorMessage(reason)
      if (version === selectionVersion.current) setError(message)
      return message
    } finally {
      setLifecycleBusy(false)
    }
  }
  const runLifecycle = async (restart: boolean): Promise<void> => {
    if (!activeSessionId || lifecycleBusy) return
    setLifecycleBusy(true)
    setError(undefined)
    try {
      if (restart) {
        const runtime = await api.restartSession(activeSessionId)
        setProjection((current) => projectDesktopEvent(current, {
          type: 'runtime_updated', runtime,
        }))
      } else await api.stopSession(activeSessionId)
    } catch (reason) {
      setError(errorMessage(reason))
    } finally {
      setLifecycleBusy(false)
    }
  }
  const send = async (agentId?: string): Promise<void> => {
    const text = targetDrafts.get(agentId).trim()
    if (!activeSessionId || (!text && imageAttachments.images.length === 0)
      || sending || !projection.detail
      || !isSessionLive(projection.detail.session)) return
    const images = imageAttachments.clearImages()
    targetDrafts.set(agentId, '')
    setSending(true)
    setError(undefined)
    try {
      if (images.length) await api.sendMessage(activeSessionId, text, agentId, images)
      else await api.sendMessage(activeSessionId, text, agentId)
    }
    catch (reason) {
      targetDrafts.set(agentId, text)
      imageAttachments.restoreImages(images)
      setError(errorMessage(reason))
    } finally { setSending(false) }
  }
  return {
    registries, projection, workspaces, activeWorkspaceId, activeSessionId,
    currentSessions: catalog.currentSessions, searchResults: catalog.searchResults,
    query, setQuery, searchRef,
    openSession, createSession, stopSession: () => runLifecycle(false),
    restartSession: () => runLifecycle(true),
    canCreate: !lifecycleBusy,
    lifecycleBusy,
    draftFor: targetDrafts.get,
    setDraftFor: targetDrafts.set,
    sending, error, send,
    images: imageAttachments.images, selectImages: imageAttachments.selectImages,
    removeImage: imageAttachments.removeImage,
  }
}
function errorMessage(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason)
}
function withoutDetail({ detail: _detail, ...projection }: DesktopProjection): DesktopProjection {
  return projection
}
