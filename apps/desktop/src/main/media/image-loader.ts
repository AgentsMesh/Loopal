import { open } from 'node:fs/promises'
import { basename } from 'node:path'
import {
  DESKTOP_IMAGE_MAX_BYTES,
  DESKTOP_IMAGE_MAX_COUNT,
  DESKTOP_IMAGE_MAX_TOTAL_BYTES,
  DesktopImageAttachmentListSchema,
  type DesktopImageAttachment,
} from '../../shared/contracts'

export async function loadSelectedImages(
  paths: readonly string[],
): Promise<DesktopImageAttachment[]> {
  if (paths.length > DESKTOP_IMAGE_MAX_COUNT) {
    throw new Error(`Attach at most ${DESKTOP_IMAGE_MAX_COUNT} images`)
  }
  const images: DesktopImageAttachment[] = []
  let total = 0
  for (const path of paths) {
    const data = await readBoundedFile(path)
    total += data.length
    if (total > DESKTOP_IMAGE_MAX_TOTAL_BYTES) {
      throw new Error('Image attachments exceed the total size limit')
    }
    const mediaType = detectImageType(data)
    if (!mediaType) throw new Error(`${basename(path)} is not a supported image`)
    images.push({
      name: basename(path).slice(0, 255), mediaType,
      data: data.toString('base64'), sizeBytes: data.length,
    })
  }
  return DesktopImageAttachmentListSchema.parse(images)
}

async function readBoundedFile(path: string): Promise<Buffer> {
  const file = await open(path, 'r')
  try {
    const stat = await file.stat()
    if (!stat.isFile() || stat.size <= 0 || stat.size > DESKTOP_IMAGE_MAX_BYTES) {
      throw new Error(`${basename(path)} must be a non-empty image under 10 MiB`)
    }
    const data = Buffer.alloc(stat.size)
    let offset = 0
    while (offset < data.length) {
      const { bytesRead } = await file.read(data, offset, data.length - offset, offset)
      if (bytesRead === 0) throw new Error(`${basename(path)} changed while it was being read`)
      offset += bytesRead
    }
    return data
  } finally {
    await file.close()
  }
}

function detectImageType(data: Buffer): DesktopImageAttachment['mediaType'] | undefined {
  if (data.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) {
    return 'image/png'
  }
  if (data[0] === 0xff && data[1] === 0xd8 && data[2] === 0xff) return 'image/jpeg'
  const prefix = data.subarray(0, 6).toString('ascii')
  if (prefix === 'GIF87a' || prefix === 'GIF89a') return 'image/gif'
  if (data.subarray(0, 4).toString('ascii') === 'RIFF'
    && data.subarray(8, 12).toString('ascii') === 'WEBP') return 'image/webp'
  return undefined
}
