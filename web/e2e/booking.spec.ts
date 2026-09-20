// A member books and cancels a session. The session is created through the
// API as the admin, so the spec does not depend on fixture dates.
import type { APIRequestContext } from '@playwright/test'
import { ADMIN, expect, MEMBER_STATE, PASSWORD, test, weekOf } from './fixtures'

test.use({ storageState: MEMBER_STATE })

interface Named {
  id: number
  name: string
}

let admin: APIRequestContext
let sessionId: number
let sessionTypeName: string
const notes = `e2e booking ${Date.now()}`
// Three days ahead, mid-morning, so it is bookable whatever the time of day
const datetime = new Date()
datetime.setDate(datetime.getDate() + 3)
datetime.setUTCHours(9, 0, 0, 0)

test.beforeAll(async ({ playwright, baseURL }) => {
  // Start from no cookies: the member storage state used by the page tests
  // must not leak into the admin's API session
  admin = await playwright.request.newContext({ baseURL, storageState: { cookies: [], origins: [] } })
  const login = await admin.post('/api/login', { data: { email: ADMIN.email, password: PASSWORD } })
  expect(login.ok(), 'admin login').toBe(true)
  expect(((await login.json()) as { roles: string[] }).roles, 'logged in as the admin').toContain('admin')

  const [sessionType] = (await (await admin.get('/api/session_types?deprecated=false')).json()) as Named[]
  sessionTypeName = sessionType.name
  const [location] = (await (await admin.get('/api/locations')).json()) as Named[]
  const [trainer] = (await (await admin.get('/api/users/list?role=trainer')).json()) as Named[]

  const created = await admin.post('/api/sessions', {
    data: {
      datetime: datetime.toISOString(),
      duration_mins: 60,
      session_type_id: sessionType.id,
      location_id: location.id,
      trainer_id: trainer.id,
      max_bookings: null,
      notes,
      cost: 0,
      booking_deadline_mins: 0
    }
  })
  expect(created.ok(), `create session: ${await created.text()}`).toBe(true)
  sessionId = (await created.json()) as number
})

test.afterAll(async () => {
  if (sessionId) {
    await admin.delete(`/api/sessions/${sessionId}`)
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
  const row = page.getByRole('row').filter({ hasText: sessionTypeName })
  await expect(row).toHaveCount(1)

  await row.getByRole('link', { name: 'Cancel' }).click()
  await expect(row).toHaveCount(0)
})
