import { mkdtemp, rm, truncate, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import { afterEach, describe, expect, it } from 'vitest'
import { DESKTOP_IMAGE_MAX_BYTES, DESKTOP_IMAGE_MAX_COUNT } from '../../shared/contracts'
import { loadSelectedImages } from './image-loader'

let directory: string | undefined

afterEach(async () => {
  if (directory) await rm(directory, { recursive: true, force: true })
  directory = undefined
})

async function fixture(name: string, bytes: readonly number[]): Promise<string> {
  directory ??= await mkdtemp(join(tmpdir(), 'loopal-images-'))
  const path = join(directory, name)
  await writeFile(path, Buffer.from(bytes))
  return path
}

describe('selected image loading', () => {
  it('detects image content and emits bounded base64 data', async () => {
    const png = await fixture('pixel.png', [137, 80, 78, 71, 13, 10, 26, 10])
    const gif = await fixture('animation.gif', [...Buffer.from('GIF89a'), 0, 0])
    await expect(loadSelectedImages([png, gif])).resolves.toEqual([
      {
        name: 'pixel.png', mediaType: 'image/png',
        data: Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]).toString('base64'), sizeBytes: 8,
      },
      {
        name: 'animation.gif', mediaType: 'image/gif',
        data: Buffer.from([...Buffer.from('GIF89a'), 0, 0]).toString('base64'), sizeBytes: 8,
      },
    ])
  })

  it('rejects spoofed, oversized, and excessive selections before routing', async () => {
    const fake = await fixture('fake.png', [...Buffer.from('not an image')])
    await expect(loadSelectedImages([fake])).rejects.toThrow('not a supported image')

    const huge = await fixture('huge.png', [137, 80, 78, 71, 13, 10, 26, 10])
    await truncate(huge, DESKTOP_IMAGE_MAX_BYTES + 1)
    await expect(loadSelectedImages([huge])).rejects.toThrow('under 10 MiB')
    await expect(loadSelectedImages(
      Array.from({ length: DESKTOP_IMAGE_MAX_COUNT + 1 }, () => fake),
    )).rejects.toThrow('Attach at most')
  })
})
