export type RendererPlatform = 'darwin' | 'windows' | 'linux'

export function detectRendererPlatform(value: string): RendererPlatform {
  const platform = value.toLowerCase()
  if (platform.startsWith('mac')) return 'darwin'
  if (platform.startsWith('win')) return 'windows'
  return 'linux'
}
