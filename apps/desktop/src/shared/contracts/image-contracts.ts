import { z } from 'zod'

export const DESKTOP_IMAGE_MAX_COUNT = 4
export const DESKTOP_IMAGE_MAX_BYTES = 10 * 1024 * 1024
export const DESKTOP_IMAGE_MAX_TOTAL_BYTES = 20 * 1024 * 1024

export const DesktopImageMediaTypeSchema = z.enum([
  'image/png', 'image/jpeg', 'image/gif', 'image/webp',
])

const base64 = /^[A-Za-z0-9+/]+={0,2}$/
const maxBase64Length = Math.ceil(DESKTOP_IMAGE_MAX_BYTES / 3) * 4

export const DesktopImageAttachmentSchema = z.object({
  name: z.string().min(1).max(255),
  mediaType: DesktopImageMediaTypeSchema,
  data: z.string().min(4).max(maxBase64Length).regex(base64),
  sizeBytes: z.number().int().positive().max(DESKTOP_IMAGE_MAX_BYTES),
}).superRefine((image, context) => {
  if (base64ByteLength(image.data) !== image.sizeBytes) {
    context.addIssue({ code: 'custom', message: 'Image size does not match its data' })
  }
})

export const DesktopImageAttachmentListSchema = z.array(DesktopImageAttachmentSchema)
  .max(DESKTOP_IMAGE_MAX_COUNT)
  .superRefine((images, context) => {
    const total = images.reduce((sum, image) => sum + image.sizeBytes, 0)
    if (total > DESKTOP_IMAGE_MAX_TOTAL_BYTES) {
      context.addIssue({ code: 'custom', message: 'Image attachments exceed the total size limit' })
    }
  })

export type DesktopImageAttachment = z.infer<typeof DesktopImageAttachmentSchema>

function base64ByteLength(value: string): number {
  const padding = value.endsWith('==') ? 2 : value.endsWith('=') ? 1 : 0
  return (value.length * 3) / 4 - padding
}
