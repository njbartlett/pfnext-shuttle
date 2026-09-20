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

// Accounts seeded by src/fixtures/users.sql (shared with the Rust tests)
export const PASSWORD = 'password'
export const ADMIN = { name: 'admin', email: 'admin@example.com' }
export const TRAINER = { name: 'trainer', email: 'trainer@example.com' }
export const MEMBER = { name: 'user1', email: 'user1@example.com' }

export const SITE_NAME = 'Another Level'

// Saved browser state (cookie plus the localStorage login mirror) per role
export const MEMBER_STATE = 'e2e/.auth/member.json'

// The sessions page's ?week= value for the week containing `date`: the
// Monday, as a local calendar date
export function weekOf(date: Date): string {
  const monday = new Date(date)
  monday.setDate(monday.getDate() - ((monday.getDay() + 6) % 7))
  return (
    String(monday.getFullYear()) +
    '-' +
    String(monday.getMonth() + 1).padStart(2, '0') +
    '-' +
    String(monday.getDate()).padStart(2, '0')
  )
}
