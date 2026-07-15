import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

export const defaultTestWorkspacePath = '/workspace/project'

export function nativeTestPath(path: string): string {
  return resolve(path)
}

export function nativeTestFileUri(path: string): string {
  return pathToFileURL(nativeTestPath(path)).href
}
