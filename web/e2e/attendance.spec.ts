// The trainer of a session records who attended. The session, with a member
// booked on it, is arranged through the API as the admin.
import type { APIRequestContext } from '@playwright/test'
import { bookMember, createSession, deleteSession, findUser, listSessionBookings, loginApi } from './api'
import { ADMIN, daysAhead, expect, MEMBER, SESSION_TYPE_NAME, test, TRAINER, weekOf } from './fixtures'

test.use({ storageState: TRAINER.state })

let admin: APIRequestContext
let sessionId: number
const notes = `e2e attendance ${Date.now()}`
const datetime = daysAhead(4)

test.beforeAll(async ({ playwright, baseURL }) => {
  admin = await loginApi(playwright, baseURL, ADMIN)
  sessionId = await createSession(admin, { datetime, notes, trainerEmail: TRAINER.email })
  await bookMember(admin, (await findUser(admin, MEMBER.email)).id, sessionId)
})

test.afterAll(async () => {
  if (sessionId) {
    await deleteSession(admin, sessionId)
  }
  await admin.dispose()
})

test('the trainer opens attendance from the session and marks a member present', async ({ page }) => {
  const sessionsUrl = `/sessions.html?week=${weekOf(datetime)}`
  await page.goto(sessionsUrl)
  const card = page.locator('.hover-expand', { hasText: notes })
  // Trainers get the attendance tool for their own sessions, not the admin menu
  await expect(card.getByRole('button', { name: 'Book', exact: true })).toBeVisible()
  await card.getByRole('link', { name: 'Attendance' }).click()

  await expect(page).toHaveURL(new RegExp(`/attendance\\.html\\?id=${sessionId}&return=`))
  await expect(page).toHaveTitle(/^Session Attendance/)
  await expect(page.getByText(`1 member(s) booked ${SESSION_TYPE_NAME} session`)).toBeVisible()

  const row = page.getByRole('row').filter({ hasText: MEMBER.name })
  const attended = row.getByRole('checkbox')
  await expect(attended).not.toBeChecked()

  await attended.check()
  await expect(attended).toBeChecked()
  // Recorded on the server, not just in the page
  await page.reload()
  await expect(row.getByRole('checkbox')).toBeChecked()
  expect((await listSessionBookings(admin, sessionId)).map((b) => [b.person_name, b.attended])).toEqual([[MEMBER.name, true]])

  await page.getByRole('button', { name: 'Clear All' }).click()
  await expect(row.getByRole('checkbox')).not.toBeChecked()
  await page.reload()
  await expect(row.getByRole('checkbox')).not.toBeChecked()

  await page.getByRole('button', { name: 'All Attended' }).click()
  await expect(row.getByRole('checkbox')).toBeChecked()

  // Closing returns to the week the tool was opened from
  await page.getByRole('button', { name: 'Close' }).click()
  await expect(page).toHaveURL(new RegExp(`${sessionsUrl.replace('?', '\\?')}$`))
  await expect(card).toBeVisible()
})
