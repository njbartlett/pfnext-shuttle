// The admin writes a post in the TinyMCE editor, publishes it, reads it
// back, checks the link preview tags Rocket adds, and deletes it.
import type { APIRequestContext } from '@playwright/test'
import { loginApi } from './api'
import { ADMIN, daysAhead, expect, isoDate, test } from './fixtures'

test.use({ storageState: ADMIN.state })

interface PostRow {
  id: number
  title: string
}

let admin: APIRequestContext
const title = `E2E post ${Date.now()}`
const encodedLocation = `${encodeURIComponent(title)}.html`
const body = 'Written by Playwright, with a lovely image to follow.'

test.beforeAll(async ({ playwright, baseURL }) => {
  admin = await loginApi(playwright, baseURL, ADMIN)
})

// Whatever the test left behind
test.afterAll(async () => {
  const posts = (await (await admin.get('/api/posts?all=true')).json()) as PostRow[]
  for (const post of posts.filter((p) => p.title === title)) {
    await admin.delete(`/api/posts/${post.id}`)
  }
  await admin.dispose()
})

test('the admin writes, publishes, reads and deletes a post', async ({ page, request }) => {
  await page.goto('/blog/index.html')
  await expect(page.getByRole('heading', { name: 'Posts' })).toBeVisible()
  await page.getByRole('link', { name: 'New Post' }).click()

  await expect(page).toHaveURL(/\/blog\/edit\/new$/)
  await expect(page.getByRole('heading', { name: 'Create Post' })).toBeVisible()
  const save = page.getByRole('button', { name: 'Save' })
  await expect(save).toBeDisabled()

  await page.getByLabel('Title').fill(title)
  // Published yesterday, so the post is live whatever the time of day
  await page.getByLabel('Publication Date').fill(isoDate(daysAhead(-1)))
  await page.getByLabel('Publication Time').fill('09:00')
  const editor = page.frameLocator('iframe.tox-edit-area__iframe').locator('body')
  await editor.click()
  await editor.fill(body)
  await expect(save).toBeEnabled()
  await save.click()

  // Saved: the editor now lives at the post's own URL
  await expect(page).toHaveURL(new RegExp(`/blog/edit/${encodedLocation.replace(/[.*+?^${}()|[\]\\%]/g, '\\$&')}$`))
  await expect(page.getByText(/^Saved at/)).toBeVisible()
  await expect(page.getByRole('heading', { name: `Edit Post – ${title}` })).toBeVisible()

  // Close opens the post
  await page.getByRole('button', { name: 'Close' }).click()
  await expect(page).toHaveURL(new RegExp(`/blog/posts/${encodedLocation.replace(/[.*+?^${}()|[\]\\%]/g, '\\$&')}$`))
  await expect(page).toHaveTitle(`${title} – Another Level`)
  await expect(page.getByRole('heading', { name: title })).toBeVisible()
  await expect(page.getByText(body)).toBeVisible()
  await expect(page.getByText(/Published on/)).toBeVisible()

  // The shell Rocket serves for the post carries its Open Graph tags
  const shell = await (await request.get(page.url())).text()
  expect(shell).toContain(`<meta property="og:title" content="${title}">`)
  expect(shell).toContain(`<meta property="og:description" content="${body}">`)
  expect(shell).toContain(`<title>${title} – Another Level</title>`)

  // Listed for everyone
  await page.goto('/blog/index.html')
  const listed = page.getByRole('link', { name: title })
  await expect(listed).toBeVisible()
  await listed.click()
  await expect(page.getByRole('heading', { name: title })).toBeVisible()

  // Delete from the post page, confirming in the modal
  await page.getByRole('button', { name: 'Delete' }).click()
  const dialog = page.locator('.modal.show')
  await expect(dialog).toContainText('Are you sure you want to delete this Post?')
  await dialog.getByRole('button', { name: 'Delete' }).click()

  await expect(page).toHaveURL(/\/blog\/index\.html\?mode=allposts$/)
  await expect(page.getByText('Showing all published and unpublished posts.')).toBeVisible()
  await expect(page.getByRole('link', { name: title })).toHaveCount(0)
})
