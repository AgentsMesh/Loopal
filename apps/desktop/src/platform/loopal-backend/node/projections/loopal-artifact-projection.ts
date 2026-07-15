import { createHash } from 'node:crypto'
import { basename, extname } from 'node:path'
import { type Artifact } from '../../../../shared/contracts'

export function projectModifiedFiles(
  sessionId: string,
  agentId: string,
  paths: readonly string[],
  createdAt: string,
): Artifact[] {
  return [...new Set(paths.map(normalizePath).filter(Boolean))].map((path) => ({
    id: `file-${digest(`${agentId}\0${path}`)}`,
    sessionId,
    title: basename(path),
    ...artifactType(path),
    uri: `loopal-workspace://${encodeURIComponent(path)}`,
    producerAgentId: agentId,
    createdAt,
  }))
}

function normalizePath(value: string): string {
  return value.trim().replace(/^\.\//, '')
}

function digest(value: string): string {
  return createHash('sha256').update(value).digest('hex').slice(0, 16)
}

function artifactType(path: string): Pick<Artifact, 'kind' | 'mediaType'> {
  const extension = extname(path).toLowerCase()
  if (['.png', '.jpg', '.jpeg', '.gif', '.webp', '.svg'].includes(extension)) {
    const subtype = extension === '.jpg' ? 'jpeg' : extension.slice(1)
    return { kind: 'image', mediaType: extension === '.svg' ? 'image/svg+xml' : `image/${subtype}` }
  }
  if (['.md', '.mdx', '.txt', '.pdf'].includes(extension)) {
    const mediaType = extension === '.pdf'
      ? 'application/pdf'
      : extension === '.txt' ? 'text/plain' : 'text/markdown'
    return { kind: 'document', mediaType }
  }
  return { kind: 'code', mediaType: 'text/plain' }
}
