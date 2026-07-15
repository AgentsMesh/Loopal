import { z } from 'zod'
import { type SessionSummary } from '../../../../shared/contracts'
import { type DesktopHostClient } from '../backend/loopal-backend-types'

const CatalogSessionSchema = z.object({
  id: z.string().min(1),
  title: z.string().min(1),
  model: z.string().min(1),
  mode: z.string().min(1),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
})

const SessionCatalogSchema = z.array(CatalogSessionSchema)
export type CatalogSession = z.infer<typeof CatalogSessionSchema>

export async function loadSessionCatalog(
  host: DesktopHostClient,
  workspaceId: string,
): Promise<readonly CatalogSession[]> {
  return SessionCatalogSchema.parse(
    await host.request('desktop/listSessions', { workspaceId }),
  )
}

export function stoppedSession(
  value: CatalogSession,
  workspaceId: string,
): SessionSummary {
  return { ...value, workspaceId, status: 'stopped' }
}

export function fallbackSession(
  sessionId: string,
  workspaceId: string,
  workspaceName: string,
  now: string,
): SessionSummary {
  return {
    id: sessionId,
    workspaceId,
    title: sessionTitle(workspaceName),
    model: 'loopal-default',
    mode: 'agent',
    status: 'stopped',
    createdAt: now,
    updatedAt: now,
  }
}

function sessionTitle(workspaceName: string): string {
  return `Loopal session · ${workspaceName}`
}
