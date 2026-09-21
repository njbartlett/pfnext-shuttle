// Routing through the real application shell: the login guard and its
// return link, logging in and out, a login that expires on a member page,
// legacy fragment links and the page titles. API calls are mocked.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, type VueWrapper } from '@vue/test-utils'
import type { Router } from 'vue-router'
import { getUserRecord, listBookings, listPolls, listSessions, login, logout } from '@pfnext/shared'
import App from './App.vue'
import { SITE_NAME } from '@/app/site'
import { apiError } from '@/stores/apiError'
import { setUser } from '@/stores/auth'
import { makeUserRecord, MEMBER, NOW } from '@/testing/fixtures'
import { mountAt, waitFor } from '@/testing/mount'

vi.mock('@pfnext/shared', () => import('@/testing/sharedMock').then((m) => m.mockedShared()))

let wrapper: VueWrapper
let router: Router

async function open(path: string) {
  ;({ wrapper, router } = await mountAt(App, path, { attachTo: document.body }))
}

function navbar() {
  return wrapper.find('nav')
}

beforeEach(() => {
  vi.setSystemTime(NOW)
  vi.clearAllMocks()
  // What the bookings and sessions pages load
  vi.mocked(listBookings).mockResolvedValue([])
  vi.mocked(listSessions).mockResolvedValue([])
  vi.mocked(getUserRecord).mockResolvedValue(makeUserRecord(MEMBER))
  vi.mocked(listPolls).mockResolvedValue([])
})

afterEach(() => {
  wrapper?.unmount()
  setUser(null)
  expect(apiError.value).toBeNull()
})

describe('App login guard', () => {
  it('sends a logged-out visitor to the login page with a return link', async () => {
    await open('/bookings.html')

    expect(router.currentRoute.value.name).toBe('login')
    expect(router.currentRoute.value.query.return).toBe('/bookings.html')
    expect(document.title).toBe(`Login – ${SITE_NAME}`)
    // Focused page: no navbar, just the form
    expect(navbar().exists()).toBe(false)
    expect(wrapper.find('#inputEmail').exists()).toBe(true)
  })

  it('returns to the requested page after logging in', async () => {
    vi.mocked(login).mockResolvedValue(MEMBER)
    await open('/bookings.html')

    await wrapper.find('#inputEmail').setValue(MEMBER.email)
    await wrapper.find('#inputPassword').setValue('password')
    await wrapper.find('form').trigger('submit')
    await waitFor(() => expect(router.currentRoute.value.path).toBe('/bookings.html'))
    await flushPromises()

    expect(login).toHaveBeenCalledWith(MEMBER.email, 'password')
    expect(document.title).toBe(`Bookings – ${SITE_NAME}`)
    expect(navbar().text()).toContain(MEMBER.name)
    expect(wrapper.find('h1').text()).toBe('Bookings')
    // The page loaded its data for the member who just logged in
    expect(listBookings).toHaveBeenCalledWith(MEMBER.id, new Date('2025-06-01T00:00:00Z'), new Date('2025-06-30T23:59:59.999Z'))
  })

  it('lets a logged-in member straight through', async () => {
    setUser(MEMBER)
    await open('/bookings.html')

    expect(router.currentRoute.value.name).toBe('bookings')
    expect(navbar().text()).toContain(MEMBER.name)
    expect(wrapper.text()).toContain(`Showing bookings for ${MEMBER.name}`)
  })

  it('leaves public pages alone', async () => {
    await open('/sessions.html')

    expect(router.currentRoute.value.name).toBe('sessions')
    expect(document.title).toBe(`Sessions – ${SITE_NAME}`)
    expect(navbar().text()).toContain('Login')
  })
})

describe('App login expiry', () => {
  it('moves to the login page when the login is cleared on a member page', async () => {
    setUser(MEMBER)
    await open('/bookings.html')

    // What the auth store does when the API answers 401
    setUser(null)
    await waitFor(() => expect(router.currentRoute.value.name).toBe('login'))

    expect(router.currentRoute.value.query.return).toBe('/bookings.html')
    expect(document.title).toBe(`Login – ${SITE_NAME}`)
  })

  it('stays put on a public page', async () => {
    setUser(MEMBER)
    await open('/sessions.html')

    setUser(null)
    await flushPromises()

    expect(router.currentRoute.value.name).toBe('sessions')
    expect(navbar().text()).toContain('Login')
  })
})

describe('App logout', () => {
  it('goes home from a member page without passing through login', async () => {
    vi.mocked(logout).mockResolvedValue(undefined)
    setUser(MEMBER)
    await open('/bookings.html')
    const visited: string[] = []
    router.afterEach((to) => visited.push(to.path))

    await wrapper.find('#logout').trigger('click')
    await waitFor(() => expect(navbar().text()).toContain('Login'))

    expect(logout).toHaveBeenCalled()
    expect(visited).toEqual(['/index.html'])
    expect(document.title).toBe(`Home – ${SITE_NAME}`)
  })

  it('stays on the sessions page', async () => {
    vi.mocked(logout).mockResolvedValue(undefined)
    setUser(MEMBER)
    await open('/sessions.html')

    await wrapper.find('#logout').trigger('click')
    await waitFor(() => expect(navbar().text()).toContain('Login'))

    expect(router.currentRoute.value.name).toBe('sessions')
  })
})

describe('App legacy links', () => {
  it('turns a fragment into query parameters', async () => {
    await open('/sessions.html#week=2025-06-02')

    expect(router.currentRoute.value.fullPath).toBe('/sessions.html?week=2025-06-02')
    expect(wrapper.text()).toContain('Showing week commencing Mon, 2 June 2025')
  })

  it('keeps the converted query in the login return link', async () => {
    await open('/edit_session.html#edit=7')

    expect(router.currentRoute.value.name).toBe('login')
    expect(router.currentRoute.value.query.return).toBe('/edit_session.html?edit=7')
  })
})

describe('App unknown pages', () => {
  it('shows Not Found inside the shell', async () => {
    await open('/nonexistent.html')

    expect(document.title).toBe(`Not Found – ${SITE_NAME}`)
    expect(wrapper.text()).toContain('could not be found')
    expect(navbar().exists()).toBe(true)
  })
})
