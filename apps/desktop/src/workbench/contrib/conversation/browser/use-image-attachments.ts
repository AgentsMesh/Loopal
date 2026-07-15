import { useState } from 'react'
import {
  type DesktopImageAttachment,
  type LoopalDesktopAPI,
} from '../../../../shared/contracts'

export function useImageAttachments(
  api: LoopalDesktopAPI,
  onError: (error: string | undefined) => void,
) {
  const [images, setImages] = useState<readonly DesktopImageAttachment[]>([])

  const selectImages = async (): Promise<void> => {
    try {
      const selected = await api.selectImages()
      setImages(selected)
      onError(undefined)
    } catch (reason) {
      onError(reason instanceof Error ? reason.message : String(reason))
    }
  }
  const removeImage = (index: number): void => {
    setImages((current) => current.filter((_image, candidate) => candidate !== index))
  }
  const clearImages = (): readonly DesktopImageAttachment[] => {
    const current = images
    setImages([])
    return current
  }

  return { images, selectImages, removeImage, clearImages, restoreImages: setImages }
}
