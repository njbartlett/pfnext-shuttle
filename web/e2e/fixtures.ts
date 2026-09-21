import { test as base } from '@playwright/test'

export { expect } from '@playwright/test'

// Only the local stack is under test. Requests to any other host (the
// Instagram embed, carousel images, map links) are aborted, so a slow or
// blocked network cannot hold up page loads or make tests time out.
export const test = base.extend({
  page: async ({ page }, use) => {
    await page.route((url) => !isLocal(url), (route) => route.abort())
    await use(page)
  }
})

export function isLocal(url: URL): boolean {
  return url.hostname === 'localhost' || url.hostname === '127.0.0.1'
}

export interface Account {
  name: string
  email: string
  // Saved browser state (cookie plus the localStorage login mirror)
  state: string
}

// Accounts seeded by src/fixtures/users.sql (shared with the Rust tests)
export const PASSWORD = 'password'
export const ADMIN: Account = { name: 'admin', email: 'admin@example.com', state: 'e2e/.auth/admin.json' }
export const TRAINER: Account = { name: 'trainer', email: 'trainer@example.com', state: 'e2e/.auth/trainer.json' }
export const MEMBER: Account = { name: 'user1', email: 'user1@example.com', state: 'e2e/.auth/member.json' }
export const ACCOUNTS = [MEMBER, TRAINER, ADMIN]

export const SITE_NAME = 'Another Level'

// Seeded by schema.sql
export const SESSION_TYPE_NAME = 'HIIT'
export const LOCATION_NAME = 'Oak Hill Park'

// A date `days` ahead at the given UTC hour: mid-morning so a session is
// bookable whatever the time of day the tests run
export function daysAhead(days: number, utcHour = 9): Date {
  const date = new Date()
  date.setDate(date.getDate() + days)
  date.setUTCHours(utcHour, 0, 0, 0)
  return date
}

// `date` as the calendar date YYYY-MM-DD, local
export function isoDate(date: Date): string {
  return (
    String(date.getFullYear()) +
    '-' +
    String(date.getMonth() + 1).padStart(2, '0') +
    '-' +
    String(date.getDate()).padStart(2, '0')
  )
}

// The sessions page's ?week= value for the week containing `date`: the
// Monday, as a local calendar date
export function weekOf(date: Date): string {
  const monday = new Date(date)
  monday.setDate(monday.getDate() - ((monday.getDay() + 6) % 7))
  return isoDate(monday)
}
