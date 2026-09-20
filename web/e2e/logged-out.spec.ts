// Every page URL, visited logged out: public pages render, member pages
// redirect to the login page with a return link, and Rocket's SPA fallback
// serves pages but not missing assets
import { expect, isLocal, SITE_NAME, test } from './fixtures'

const PUBLIC_PAGES = [
  { path: '/', title: 'Home', text: 'Latest Instagram Posts' },
  { path: '/index.html', title: 'Home', text: 'Latest Instagram Posts' },
  { path: '/about.html', title: 'About', text: 'Session Types' },
  { path: '/pricing.html', title: 'Pricing', text: 'Claim Free Trial' },
  { path: '/sessions.html', title: 'Sessions', text: 'Showing week commencing' },
  { path: '/login.html', title: 'Login', text: 'Forgotten' },
  { path: '/register.html', title: 'Register', text: 'Register User' },
  { path: '/passwordreset.html?email=a@b.c', title: 'Password Reset', text: 'Update Password' }
]

const MEMBER_PAGES = [
  '/challenges.html',
  '/activities.html',
  '/polls.html',
  '/bookings.html',
  '/profile.html',
  '/attendance.html?id=1',
  '/feedback.html?id=1',
  '/edit_session.html',
  '/bulk_sessions.html',
  '/polls_admin.html',
  '/members.html',
  '/logs.html',
  '/loginlist.html',
  '/stats.html',
  '/sessions_report.html'
]

test.describe('public pages', () => {
  for (const page of PUBLIC_PAGES) {
    test(`${page.path} renders`, async ({ page: browserPage }) => {
      const errors: string[] = []
      browserPage.on('pageerror', (error) => errors.push(error.message))
      browserPage.on('console', (message) => {
        // Aborted external requests (see fixtures.ts) report as failed loads
        const source = message.location().url
        const external = source !== '' && !isLocal(new URL(source))
        if (message.type() === 'error' && !external) {
          errors.push(message.text())
        }
      })

      await browserPage.goto(page.path)
      await expect(browserPage).toHaveTitle(`${page.title} – ${SITE_NAME}`)
      await expect(browserPage.getByText(page.text).first()).toBeVisible()
      expect(errors).toEqual([])
    })
  }

  test('an unknown page shows Not Found from the router', async ({ page }) => {
    await page.goto('/nonexistent.html')
    await expect(page).toHaveTitle(`Not Found – ${SITE_NAME}`)
    await expect(page.getByText('could not be found')).toBeVisible()
  })
})

test.describe('member pages logged out', () => {
  for (const path of MEMBER_PAGES) {
    test(`${path} redirects to login with a return link`, async ({ page }) => {
      await page.goto(path)
      await expect(page).toHaveTitle(`Login – ${SITE_NAME}`)
      const returnTo = new URL(page.url()).searchParams.get('return')
      expect(returnTo).toBe(path)
    })
  }
})

test.describe('legacy links', () => {
  test('a fragment week link becomes a query parameter', async ({ page }) => {
    await page.goto('/sessions.html#week=2025-06-02')
    await expect(page).toHaveURL(/\/sessions\.html\?week=2025-06-02$/)
    await expect(page.getByText('Showing week commencing Mon, 2 June 2025')).toBeVisible()
  })

  test('a fragment edit link survives the login redirect', async ({ page }) => {
    await page.goto('/edit_session.html#edit=7')
    await expect(page).toHaveTitle(`Login – ${SITE_NAME}`)
    expect(new URL(page.url()).searchParams.get('return')).toBe('/edit_session.html?edit=7')
  })
})

test.describe('server routing', () => {
  test('serves static files and the SPA shell only for pages', async ({ request }) => {
    expect((await request.get('/img/banner.svg')).status()).toBe(200)
    expect((await request.get('/styles/al.css')).status()).toBe(200)
    expect((await request.get('/some/deep/page')).status()).toBe(200)

    expect((await request.get('/img/missing.png')).status()).toBe(404)
    expect((await request.get('/blog/nonexistent.html')).status()).toBe(404)

    const api = await request.get('/api/nonexistent')
    expect(api.status()).toBe(404)
    expect(await api.json()).toMatchObject({ message: 'not found' })
  })
})
