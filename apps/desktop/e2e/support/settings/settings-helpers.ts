import { expect, type Page } from '@playwright/test'

export type SettingsSection =
  | 'appearance'
  | 'loopal'
  | 'providers'
  | 'mcp'
  | 'agent'
  | 'runtime'
  | 'federation'
  | 'skills'

export async function selectSettingsSection(
  page: Page,
  section: SettingsSection,
): Promise<void> {
  const navigation = page.getByTestId('settings-navigation')
  await expect(navigation).toBeVisible()
  const target = navigation.locator(`[role="tab"][data-section="${section}"]`)
  await target.click()
  await expect(target).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByTestId('settings-section-content')).toBeVisible()
}
