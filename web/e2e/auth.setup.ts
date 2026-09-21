// Logs in once per role through the real login page and saves the browser
// state, so specs start authenticated without repeating the form
import { ACCOUNTS, expect, PASSWORD, SITE_NAME, test as setup } from './fixtures'

for (const account of ACCOUNTS) {
  setup(`log in as ${account.name}`, async ({ page }) => {
    await page.goto('/login.html')
    await page.getByLabel('Email Address:').fill(account.email)
    await page.getByLabel('Password').fill(PASSWORD)
    await page.getByRole('button', { name: 'Login' }).click()

    // The login page returns to the home page by default
    await expect(page).toHaveTitle(`Home – ${SITE_NAME}`)
    await expect(page.getByRole('navigation')).toContainText(account.name)

    await page.context().storageState({ path: account.state })
  })
}
