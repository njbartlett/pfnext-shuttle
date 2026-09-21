// A member books and cancels a session. The session is created through the
// API as the admin, so the spec does not depend on fixture dates.
import type { APIRequestContext } from '@playwright/test'
import { createSession, deleteSession, loginApi } from './api'
import { ADMIN, daysAhead, expect, MEMBER, SESSION_TYPE_NAME, test, weekOf } from './fixtures'

test.use({ storageState: MEMBER.state })

let admin: APIRequestContext
let sessionId: number
const notes = `e2e booking ${Date.now()}`
const datetime = daysAhead(3)

test.beforeAll(async ({ playwright, baseURL }) => {
  admin = await loginApi(playwright, baseURL, ADMIN)
  sessionId = await createSession(admin, { datetime, notes })
})

test.afterAll(async () => {
  if (sessionId) {
    await deleteSession(admin, sessionId)
  }
  await admin.dispose()
})

test('a member books and then cancels a free session', async ({ page }) => {
  await page.goto(`/sessions.html?week=${weekOf(datetime)}`)

  // Calendar view is the default; each session is a card
  const card = page.locator('.hover-expand', { hasText: notes })
  await expect(card).toBeVisible()

  await card.getByRole('button', { name: 'Book', exact: true }).click()
  const booked = card.getByRole('button', { name: /Booked!/ })
  await expect(booked).toBeVisible()

  await booked.click()
  await card.getByRole('link', { name: 'Cancel' }).click()
  await expect(card.getByRole('button', { name: 'Book', exact: true })).toBeVisible()
  await expect(booked).toHaveCount(0)
})

test('a booking is listed on the bookings page until cancelled there', async ({ page }) => {
  await page.goto(`/sessions.html?week=${weekOf(datetime)}`)
  const card = page.locator('.hover-expand', { hasText: notes })
  await card.getByRole('button', { name: 'Book', exact: true }).click()
  await expect(card.getByRole('button', { name: /Booked!/ })).toBeVisible()

  // The bookings page pages by month; pick the session's month explicitly
  await page.goto('/bookings.html')
  await expect(page).toHaveTitle(/^Bookings/)
  const month = datetime.toLocaleDateString('en-GB', { month: 'short', year: 'numeric' })
  await page.getByRole('button', { name: month, exact: true }).click()
  await expect(page.getByText(`in ${month}`)).toBeVisible()
  const row = page.getByRole('row').filter({ hasText: SESSION_TYPE_NAME })
  await expect(row).toHaveCount(1)

  await row.getByRole('link', { name: 'Cancel' }).click()
  await expect(row).toHaveCount(0)
})
