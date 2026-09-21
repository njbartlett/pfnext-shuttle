// The admin creates a session in the editor, copies it from the sessions
// page, and deletes both again.
import type { APIRequestContext } from '@playwright/test'
import { deleteSession, listSessionsInWeek, loginApi } from './api'
import { ADMIN, daysAhead, expect, isoDate, LOCATION_NAME, SESSION_TYPE_NAME, test, TRAINER, weekOf } from './fixtures'

test.use({ storageState: ADMIN.state })

let admin: APIRequestContext
const notes = `e2e admin ${Date.now()}`
const date = daysAhead(5)

test.beforeAll(async ({ playwright, baseURL }) => {
  admin = await loginApi(playwright, baseURL, ADMIN)
})

// Whatever the test left behind
test.afterAll(async () => {
  for (const session of await listSessionsInWeek(admin, date)) {
    if (session.notes === notes) {
      await deleteSession(admin, session.id)
    }
  }
  await admin.dispose()
})

test('the admin creates a session, copies it and deletes both', async ({ page }) => {
  await page.goto('/edit_session.html')
  await expect(page.getByRole('heading', { name: 'Create New Session' })).toBeVisible()
  const save = page.getByRole('button', { name: 'Save', exact: true })
  await expect(save).toBeDisabled()

  await page.getByLabel('Date').fill(isoDate(date))
  await page.getByLabel('Time').fill('09:00')
  await page.getByLabel('Duration').fill('45')
  await page.getByLabel('Type').selectOption({ label: SESSION_TYPE_NAME })
  // Choosing the type brings in its cost
  await expect(page.getByLabel('Credits cost')).toHaveValue('1')
  await page.getByLabel('Trainer').selectOption({ label: TRAINER.name })
  await page.getByLabel('Location').selectOption({ label: LOCATION_NAME })
  await page.getByLabel('Notes').fill(notes)
  await expect(save).toBeEnabled()
  await save.click()

  // Saved: the editor now edits the new session
  await expect(page).toHaveURL(/\/edit_session\.html\?edit=\d+$/)
  await expect(page.getByRole('heading', { name: 'Edit Session' })).toBeVisible()
  const createdId = Number(new URL(page.url()).searchParams.get('edit'))

  // Copy it from the sessions page
  const sessionsUrl = `/sessions.html?week=${weekOf(date)}`
  await page.goto(sessionsUrl)
  const cards = page.locator('.hover-expand', { hasText: notes })
  await expect(cards).toHaveCount(1)
  await expect(cards.first()).toContainText(`${TRAINER.name}`)
  await cards.first().locator('.dropup > button.dropdown-toggle').click()
  await cards.first().getByRole('link', { name: 'Copy' }).click()

  await expect(page).toHaveURL(new RegExp(`/edit_session\\.html\\?copy=${createdId}&return=`))
  await expect(page.getByRole('heading', { name: 'Create New Session' })).toBeVisible()
  await expect(page.getByLabel('Notes')).toHaveValue(notes)
  await expect(page.getByLabel('Date')).toHaveValue(isoDate(date))
  await expect(page.getByLabel('Duration')).toHaveValue('45')
  await page.getByLabel('Time').fill('10:00')
  await save.click()
  // Now editing the copy; the return link is kept
  await expect(page).toHaveURL(/\/edit_session\.html\?.*[?&]edit=\d+/)
  await expect(page.getByRole('heading', { name: 'Edit Session' })).toBeVisible()
  expect(Number(new URL(page.url()).searchParams.get('edit'))).not.toBe(createdId)

  // Both sessions are there, then neither
  await page.goto(sessionsUrl)
  await expect(cards).toHaveCount(2)
  for (const remaining of [1, 0]) {
    await cards.first().locator('.dropup > button.dropdown-toggle').click()
    await cards.first().getByRole('button', { name: 'Delete' }).click()
    const dialog = page.locator('.modal.show')
    await expect(dialog).toContainText('Are you sure you want to delete the following session?')
    await dialog.getByRole('button', { name: 'Delete' }).click()
    await expect(cards).toHaveCount(remaining)
  }
})
