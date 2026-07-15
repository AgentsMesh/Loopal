import { expect, test } from '@playwright/test'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, send } from '../../support/runtime/llm-e2e-helpers'

test('selects, removes, and sends captioned and image-only model turns', async () => {
  const desktop = await launchDesktop('real', 'provider-user-images')
  try {
    const page = desktop.page
    await desktop.app.evaluate(async ({ dialog }, path) => {
      dialog.showOpenDialog = async () => ({
        canceled: false, filePaths: [path], bookmarks: [],
      })
    }, join(desktop.project, 'pixel.png'))
    await ready(page)

    await page.getByRole('button', { name: 'Attach images' }).click()
    let pending = page.getByTestId('pending-image-attachments')
    await expect(pending).toContainText('pixel.png')
    await page.getByRole('button', { name: 'Remove pixel.png' }).click()
    await expect(pending).toHaveCount(0)

    await page.getByRole('button', { name: 'Attach images' }).click()
    pending = page.getByTestId('pending-image-attachments')
    await expect(pending).toContainText('pixel.png')
    await send(page, 'Describe the attached pixel')
    await expect(pending).toHaveCount(0)
    const conversation = page.getByTestId('conversation')
    await expect(conversation).toContainText('1 image attachment(s)')
    await expect(conversation).toContainText(
      'The caption and image reached the production provider.', { timeout: 20_000 },
    )

    await ready(page)
    await page.getByRole('button', { name: 'Attach images' }).click()
    await expect(page.getByTestId('pending-image-attachments')).toContainText('pixel.png')
    await page.getByRole('button', { name: 'Send' }).click()
    await expect(conversation).toContainText(
      'The image-only turn also reached the production provider.', { timeout: 20_000 },
    )
    const requests = await desktop.llm!.requests()
    expect(requests).toHaveLength(2)
    expect(requests[0]).toMatchObject({ lastUserText: 'Describe the attached pixel',
      imageBlockCount: 1 })
    expect(requests[1]).toMatchObject({ lastUserText: '', imageBlockCount: 2 })
    await expect.poll(() => desktop.llm!.state()).toMatchObject({
      served: 2, remaining: 0, verified: true,
    })
  } finally {
    await closeDesktop(desktop)
  }
})
