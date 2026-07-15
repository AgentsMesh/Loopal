import { expect, test } from '@playwright/test'
import { join } from 'node:path'
import { closeDesktop, launchDesktop } from '../../support/electron/electron-fixture'
import { ready, send } from '../../support/runtime/llm-e2e-helpers'
import { type E2eProvider, providerModel } from '../../support/providers/provider-e2e-fixture'

const providers: readonly E2eProvider[] = [
  'anthropic', 'openai', 'openai_compat', 'google',
]

for (const provider of providers) {
  test(`${provider} sends a user image through its production adapter`, async () => {
    const desktop = await launchDesktop('real', 'provider-image-matrix', {}, provider)
    try {
      await desktop.app.evaluate(async ({ dialog }, path) => {
        dialog.showOpenDialog = async () => ({
          canceled: false, filePaths: [path], bookmarks: [],
        })
      }, join(desktop.project, 'pixel.png'))
      await ready(desktop.page)
      await desktop.page.getByRole('button', { name: 'Attach images' }).click()
      await expect(desktop.page.getByTestId('pending-image-attachments'))
        .toContainText('pixel.png')
      await send(desktop.page, 'Exercise provider image input')
      await expect(desktop.page.getByTestId('conversation')).toContainText(
        'Provider image adapter completed its model turn.', { timeout: 20_000 },
      )
      await ready(desktop.page)
      expect((await desktop.llm!.requests())[0]).toMatchObject({
        model: providerModel(provider), imageBlockCount: 1,
      })
      await expect.poll(() => desktop.llm!.state()).toMatchObject({
        served: 1, remaining: 0, verified: true,
      })
    } finally {
      await closeDesktop(desktop)
    }
  })
}
