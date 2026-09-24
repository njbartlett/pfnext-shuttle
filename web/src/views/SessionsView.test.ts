// The sessions page with a logged-in member: the alerts, the booking flow
// (including the credits confirmation), cancelling, the waitlist, the
// ?week= parameter and the one-day calendar of a narrow screen. API calls
// are mocked; see testing/sharedMock.ts.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, type BaseWrapper, type DOMWrapper, type VueWrapper } from '@vue/test-utils'
import type { Router } from 'vue-router'
import {
  ApiError, bookSession, cancelBooking, getUserRecord, joinWaitlist, listPolls, listSessions, type Session
} from '@pfnext/shared'
import SessionsView from './SessionsView.vue'
import { apiError, clearApiError } from '@/stores/apiError'
import { setUser } from '@/stores/auth'
import { makeSession, makeUserRecord, MEMBER, NOW } from '@/testing/fixtures'
import { mountAt } from '@/testing/mount'

vi.mock('@pfnext/shared', () => import('@/testing/sharedMock').then((m) => m.mockedShared()))

let wrapper: VueWrapper
let router: Router

async function mountSessions(path = '/sessions.html') {
  ;({ wrapper, router } = await mountAt(SessionsView, path, { attachTo: document.body }))
}

// The session's card in the calendar view
function card(): DOMWrapper<Element> {
  return wrapper.find('.hover-expand')
}

function buttonIn(parent: BaseWrapper<Node>, label: string | RegExp): DOMWrapper<Element> {
  const button = parent.findAll('button').find((b) => (typeof label === 'string' ? b.text() === label : label.test(b.text())))
  if (!button) {
    throw new Error(`No button "${label}" in: ${parent.text()}`)
  }
  return button
}

// Bootstrap modals open and close on a timer and ignore hide() while still
// opening, so tests wait for the events rather than the class. The events
// bubble to the document.
function nextModalEvent(name: 'shown.bs.modal' | 'hidden.bs.modal'): Promise<DOMWrapper<Element>> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`No ${name} event within 1s`)), 1000)
    document.addEventListener(
      name,
      (event) => {
        clearTimeout(timer)
        const modal = wrapper.findAll('.modal').find((m) => m.element === event.target)
        if (modal) {
          resolve(modal)
        } else {
          reject(new Error(`${name} fired on an element outside the view`))
        }
      },
      { once: true }
    )
  })
}

// Clicks a button in the shown modal and waits for the modal to be gone
async function dismiss(modal: DOMWrapper<Element>, label: string) {
  const hidden = nextModalEvent('hidden.bs.modal')
  await buttonIn(modal, label).trigger('click')
  await hidden
}

// The screen width as the view sees it through matchMedia (jsdom's stub in
// setup.ts never matches, so the default is a wide screen). resizeScreen()
// fires the change listeners as rotating a phone would.
const originalMatchMedia = window.matchMedia
let screenListeners: ((event: MediaQueryListEvent) => void)[] = []

function stubScreen(narrow: boolean) {
  screenListeners = []
  window.matchMedia = (query: string) =>
    ({
      matches: narrow && query.startsWith('(max-width:'),
      media: query,
      addEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => screenListeners.push(listener),
      removeEventListener: () => undefined
    }) as unknown as MediaQueryList
}

async function resizeScreen(narrow: boolean) {
  screenListeners.forEach((listener) => listener({ matches: narrow } as MediaQueryListEvent))
  await flushPromises()
}

// A one-finger touch moving by (dx, dy). jsdom has no Touch constructor, so
// the touches are plain objects on plain events.
function swipe(element: Element, dx: number, dy: number) {
  const point = (x: number, y: number) => [{ clientX: x, clientY: y }]
  const start = new Event('touchstart')
  Object.defineProperty(start, 'touches', { value: point(200, 200) })
  element.dispatchEvent(start)
  const end = new Event('touchend')
  Object.defineProperty(end, 'touches', { value: [] })
  Object.defineProperty(end, 'changedTouches', { value: point(200 + dx, 200 + dy) })
  element.dispatchEvent(end)
}

function pagerLabels(): string[] {
  return wrapper
    .find('.btn-group')
    .findAll('button')
    .map((b) => b.text())
    .filter((text) => text !== '«' && text !== '»')
}

beforeEach(() => {
  vi.setSystemTime(NOW)
  vi.clearAllMocks()
  vi.mocked(listSessions).mockResolvedValue([])
  vi.mocked(getUserRecord).mockResolvedValue(makeUserRecord(MEMBER))
  vi.mocked(listPolls).mockResolvedValue([])
  setUser(MEMBER)
})

afterEach(() => {
  wrapper?.unmount()
  setUser(null)
  window.matchMedia = originalMatchMedia
  // Any unmocked or failed API call would have landed in the error banner
  expect(apiError.value).toBeNull()
})

describe('SessionsView logged out', () => {
  it('lists the week with a login prompt and no booking controls', async () => {
    setUser(null)
    vi.mocked(listSessions).mockResolvedValue([makeSession()])
    await mountSessions()

    expect(listSessions).toHaveBeenCalledTimes(1)
    expect(listSessions).toHaveBeenCalledWith(new Date('2025-06-02T00:00:00Z'), new Date('2025-06-09T00:00:00Z'))
    expect(getUserRecord).not.toHaveBeenCalled()
    expect(listPolls).not.toHaveBeenCalled()

    expect(wrapper.text()).toContain('You are not logged in.')
    expect(wrapper.find('a.btn-success').attributes('href')).toBe('/login.html?return=/sessions.html')
    expect(wrapper.text()).toContain('Showing week commencing Mon, 2 June 2025. Found 1 session(s)')
    // Times are the venue's (BST in June)
    expect(card().text()).toContain('10:00 HIIT')
    expect(card().find('button').exists()).toBe(false)
  })
})

describe('SessionsView alerts', () => {
  it('asks for an emergency contact when the profile has none', async () => {
    vi.mocked(getUserRecord).mockResolvedValue(makeUserRecord(MEMBER, { emergency_name: null, emergency_phone: null }))
    await mountSessions()

    const alert = wrapper.find('.alert-warning')
    expect(alert.text()).toContain('provide an emergency contact')
    expect(alert.find('a').attributes('href')).toBe('/profile.html')
  })

  it('shows nothing about the emergency contact when it is filled in', async () => {
    await mountSessions()
    expect(wrapper.find('.alert-warning').exists()).toBe(false)
  })

  it('points to an open poll the member has not voted in', async () => {
    const poll = { id: 4, question: 'Which day suits?', limit_per_person: 1, description: null, open: true }
    vi.mocked(listPolls).mockResolvedValue([
      { poll, votes: [] },
      { poll: { ...poll, id: 5, question: 'Already voted' }, votes: [{ id: 1, poll_id: 5, person_id: MEMBER.id, person_name: '', person_email: '', value: 'x' }] },
      { poll: { ...poll, id: 6, question: 'Closed', open: false }, votes: [] }
    ])
    await mountSessions()

    const alerts = wrapper.findAll('.alert-info')
    expect(alerts).toHaveLength(1)
    expect(alerts[0].text()).toContain('Which day suits?')
    expect(alerts[0].find('a').attributes('href')).toBe('/polls.html#poll-4')
  })
})

describe('SessionsView booking', () => {
  it('books a free session in place', async () => {
    vi.mocked(listSessions).mockResolvedValue([makeSession()])
    vi.mocked(bookSession).mockResolvedValue({ outcome: 'booked' })
    await mountSessions()

    await buttonIn(card(), 'Book').trigger('click')
    await flushPromises()

    expect(bookSession).toHaveBeenCalledWith(MEMBER.id, 7, 0, 0)
    expect(buttonIn(card(), /Booked!/).exists()).toBe(true)
    expect(card().find('.badge').text()).toBe('2')
  })

  it('asks before spending credits and books with them once confirmed', async () => {
    vi.mocked(listSessions).mockResolvedValue([makeSession({ cost: 2 })])
    vi.mocked(bookSession)
      .mockResolvedValueOnce({ outcome: 'credits_required', cost: 2 })
      .mockResolvedValueOnce({ outcome: 'booked' })
    await mountSessions()

    const shown = nextModalEvent('shown.bs.modal')
    await buttonIn(card(), 'Book').trigger('click')
    const modal = await shown

    // Not booked yet: the confirmation modal is up instead
    expect(buttonIn(card(), 'Book').exists()).toBe(true)
    expect(modal.text()).toContain('Do you want to use 2 of your pay-as-you-go credits')

    await dismiss(modal, 'Yes')
    await flushPromises()

    expect(vi.mocked(bookSession).mock.calls).toEqual([
      [MEMBER.id, 7, 0, 2],
      [MEMBER.id, 7, 2, 2]
    ])
    expect(buttonIn(card(), /Booked!/).exists()).toBe(true)
  })

  it('leaves the session unbooked and reports a failed booking', async () => {
    vi.mocked(listSessions).mockResolvedValue([makeSession()])
    vi.mocked(bookSession).mockRejectedValue(new ApiError(400, 'error', 'No membership or credits'))
    await mountSessions()

    await buttonIn(card(), 'Book').trigger('click')
    await flushPromises()

    expect(apiError.value?.message).toBe('No membership or credits')
    expect(buttonIn(card(), 'Book').exists()).toBe(true)
    expect(card().find('.badge').text()).toBe('1')
    clearApiError()
  })

  it('cancels a booking from the Booked! menu', async () => {
    vi.mocked(listSessions).mockResolvedValue([makeSession({ booked: true, booking_count: 2 })])
    vi.mocked(cancelBooking).mockResolvedValue(undefined)
    await mountSessions()

    const cancel = card().find('a.dropdown-item')
    expect(cancel.text()).toBe('Cancel')
    await cancel.trigger('click')
    await flushPromises()

    expect(cancelBooking).toHaveBeenCalledWith(MEMBER.id, 7)
    expect(buttonIn(card(), 'Book').exists()).toBe(true)
    expect(card().find('.badge').text()).toBe('1')
  })

  it('joins the waitlist of a full session and shows the position', async () => {
    const full: Session = makeSession({ max_booking_count: 1, booking_count: 1 })
    vi.mocked(listSessions).mockResolvedValueOnce([full]).mockResolvedValueOnce([{ ...full, waitlist_rank: 3 }])
    vi.mocked(joinWaitlist).mockResolvedValue({
      id: 1, rank: 3, person_id: MEMBER.id, person_name: MEMBER.name, person_email: MEMBER.email, person_status: null, session_id: 7
    })
    await mountSessions()

    const shown = nextModalEvent('shown.bs.modal')
    await buttonIn(card(), 'Join Waitlist').trigger('click')
    const modal = await shown
    await flushPromises()

    expect(joinWaitlist).toHaveBeenCalledWith(MEMBER.id, 7)
    expect(modal.text()).toContain('You have joined the waiting list at position')
    expect(modal.find('.fs-1').text()).toBe('3')
    // The list is reloaded so the card shows the new position
    expect(listSessions).toHaveBeenCalledTimes(2)
    expect(buttonIn(card(), /On Waitlist/).text()).toContain('(3)')

    await dismiss(modal, 'Got it!')
  })
})

describe('SessionsView week selection', () => {
  it('opens on the week in the URL with a single load', async () => {
    await mountSessions('/sessions.html?week=2025-06-16')

    expect(listSessions).toHaveBeenCalledTimes(1)
    expect(listSessions).toHaveBeenCalledWith(new Date('2025-06-16T00:00:00Z'), new Date('2025-06-23T00:00:00Z'))
    expect(wrapper.text()).toContain('Showing week commencing Mon, 16 June 2025')
    expect(buttonIn(wrapper, '16 – 22 Jun 2025').classes()).toContain('btn-primary')
  })

  it('reflects the chosen week in the URL and reloads', async () => {
    await mountSessions('/sessions.html?week=2025-06-16')

    await buttonIn(wrapper, 'This Week').trigger('click')
    await flushPromises()

    expect(router.currentRoute.value.query.week).toBeUndefined()
    expect(listSessions).toHaveBeenLastCalledWith(new Date('2025-06-02T00:00:00Z'), new Date('2025-06-09T00:00:00Z'))

    await buttonIn(wrapper, 'Next Week').trigger('click')
    await flushPromises()

    expect(router.currentRoute.value.query.week).toBe('2025-06-09')
    expect(listSessions).toHaveBeenLastCalledWith(new Date('2025-06-09T00:00:00Z'), new Date('2025-06-16T00:00:00Z'))
  })

  it('keeps the current week up with a spinner over it until the next one loads', async () => {
    vi.mocked(listSessions).mockResolvedValue([makeSession()])
    await mountSessions()
    expect(wrapper.find('.loading-overlay').exists()).toBe(false)

    let reply: (sessions: Session[]) => void = () => undefined
    vi.mocked(listSessions).mockImplementation(() => new Promise((resolve) => (reply = resolve)))
    await buttonIn(wrapper, 'Next Week').trigger('click')
    await flushPromises()

    // The old week is still up, with its own days, under the spinner
    expect(wrapper.find('.loading-overlay .spinner-border').exists()).toBe(true)
    expect(card().text()).toContain('10:00 HIIT')
    expect(wrapper.text()).toContain('Showing week commencing Mon, 2 June 2025. Found 1 session(s)')

    reply([])
    await flushPromises()

    expect(wrapper.find('.loading-overlay').exists()).toBe(false)
    expect(card().exists()).toBe(false)
    expect(wrapper.text()).toContain('Showing week commencing Mon, 9 June 2025. Found 0 session(s)')
  })

  it('drops the spinner when the load fails', async () => {
    await mountSessions()
    vi.mocked(listSessions).mockRejectedValue(new ApiError(500, 'error', 'Database down'))

    await buttonIn(wrapper, 'Next Week').trigger('click')
    await flushPromises()

    expect(apiError.value?.message).toBe('Database down')
    expect(wrapper.find('.loading-overlay').exists()).toBe(false)
    clearApiError()
  })
})

// NOW is Wednesday 4 June 2025; the fixture session is on Thursday the 5th
describe('SessionsView calendar on a narrow screen', () => {
  it('shows one day at a time, starting from Today', async () => {
    stubScreen(true)
    vi.mocked(listSessions).mockResolvedValue([makeSession()])
    await mountSessions()

    expect(listSessions).toHaveBeenCalledTimes(1)
    expect(listSessions).toHaveBeenCalledWith(new Date('2025-06-04T00:00:00Z'), new Date('2025-06-05T00:00:00Z'))
    expect(pagerLabels()).toEqual(['Today', 'Tomorrow', 'Fri 6 Jun'])
    expect(buttonIn(wrapper, 'Today').classes()).toContain('btn-primary')
    expect(wrapper.findAll('thead th')).toHaveLength(1)
    expect(wrapper.find('thead th').text()).toBe('Wednesday, 4 June 2025')
    expect(wrapper.text()).toContain('Showing Wed, 4 June 2025. Found 1 session(s) for selected day.')

    await buttonIn(wrapper, 'Tomorrow').trigger('click')
    await flushPromises()

    expect(listSessions).toHaveBeenLastCalledWith(new Date('2025-06-05T00:00:00Z'), new Date('2025-06-06T00:00:00Z'))
    expect(router.currentRoute.value.query).toEqual({ day: '2025-06-05' })
    expect(wrapper.find('thead th').text()).toBe('Thursday, 5 June 2025')
    expect(card().text()).toContain('10:00 HIIT')
  })

  it('labels other days with their date', async () => {
    stubScreen(true)
    await mountSessions()

    await buttonIn(wrapper, '»').trigger('click')
    expect(pagerLabels()).toEqual(['Sat 7 Jun', 'Sun 8 Jun', 'Mon 9 Jun'])

    // Yesterday is a step back, in the block before today's
    await buttonIn(wrapper, '«').trigger('click')
    await buttonIn(wrapper, '«').trigger('click')
    expect(pagerLabels()).toEqual(['Sun 1 Jun', 'Mon 2 Jun', 'Yesterday'])

    await buttonIn(wrapper, 'Now').trigger('click')
    expect(pagerLabels()).toEqual(['Today', 'Tomorrow', 'Fri 6 Jun'])
  })

  it('opens a ?week= link on the Monday of that week, or today for this week', async () => {
    stubScreen(true)
    await mountSessions('/sessions.html?week=2025-06-16')
    expect(listSessions).toHaveBeenCalledTimes(1)
    expect(listSessions).toHaveBeenCalledWith(new Date('2025-06-16T00:00:00Z'), new Date('2025-06-17T00:00:00Z'))
    expect(buttonIn(wrapper, 'Mon 16 Jun').classes()).toContain('btn-primary')
    wrapper.unmount()

    await mountSessions('/sessions.html?week=2025-06-02')
    expect(listSessions).toHaveBeenLastCalledWith(new Date('2025-06-04T00:00:00Z'), new Date('2025-06-05T00:00:00Z'))
    expect(buttonIn(wrapper, 'Today').classes()).toContain('btn-primary')
  })

  it('opens a ?day= link on the week of that day on a wide screen', async () => {
    await mountSessions('/sessions.html?day=2025-06-18')
    expect(listSessions).toHaveBeenCalledTimes(1)
    expect(listSessions).toHaveBeenCalledWith(new Date('2025-06-16T00:00:00Z'), new Date('2025-06-23T00:00:00Z'))
    expect(buttonIn(wrapper, '16 – 22 Jun 2025').classes()).toContain('btn-primary')
  })

  it('keeps the selected period when the screen is rotated', async () => {
    stubScreen(false)
    await mountSessions('/sessions.html?week=2025-06-16')

    await resizeScreen(true)
    expect(listSessions).toHaveBeenLastCalledWith(new Date('2025-06-16T00:00:00Z'), new Date('2025-06-17T00:00:00Z'))
    expect(router.currentRoute.value.query).toEqual({ day: '2025-06-16' })
    expect(pagerLabels()).toEqual(['Mon 16 Jun', 'Tue 17 Jun', 'Wed 18 Jun'])

    // Another day of the same week goes back to that week
    await buttonIn(wrapper, 'Tue 17 Jun').trigger('click')
    await flushPromises()
    await resizeScreen(false)
    expect(listSessions).toHaveBeenLastCalledWith(new Date('2025-06-16T00:00:00Z'), new Date('2025-06-23T00:00:00Z'))
    expect(router.currentRoute.value.query).toEqual({ week: '2025-06-16' })
    expect(listSessions).toHaveBeenCalledTimes(4)
  })

  it('pages with horizontal swipes, sliding the way it was paged', async () => {
    stubScreen(true)
    await mountSessions()
    const frame = wrapper.find('.slide-frame')

    // Swiping left moves on to tomorrow, and the tables slide forward once
    // it has loaded (Vue Test Utils stubs the transition, keeping its name)
    swipe(frame.element, -80, 5)
    await flushPromises()
    expect(listSessions).toHaveBeenLastCalledWith(new Date('2025-06-05T00:00:00Z'), new Date('2025-06-06T00:00:00Z'))
    expect(buttonIn(wrapper, 'Tomorrow').classes()).toContain('btn-primary')
    expect(wrapper.find('transition-stub').attributes('name')).toBe('slide-forward')

    // Three swipes right: today, yesterday, then beyond the visible block,
    // which slides along with it
    for (let i = 0; i < 3; i++) {
      swipe(frame.element, 80, 0)
      await flushPromises()
    }
    expect(listSessions).toHaveBeenLastCalledWith(new Date('2025-06-02T00:00:00Z'), new Date('2025-06-03T00:00:00Z'))
    expect(pagerLabels()).toEqual(['Sun 1 Jun', 'Mon 2 Jun', 'Yesterday'])
    expect(buttonIn(wrapper, 'Mon 2 Jun').classes()).toContain('btn-primary')
    expect(wrapper.find('transition-stub').attributes('name')).toBe('slide-back')

    // A mostly vertical drag is a scroll, not a swipe
    swipe(frame.element, 60, 120)
    await flushPromises()
    expect(listSessions).toHaveBeenCalledTimes(5)
  })

  it('selects only the first day of the next block when swiping on from the last', async () => {
    stubScreen(true)
    await mountSessions()

    await buttonIn(wrapper, 'Fri 6 Jun').trigger('click')
    swipe(wrapper.find('.slide-frame').element, -80, 0)
    await flushPromises()

    expect(pagerLabels()).toEqual(['Sat 7 Jun', 'Sun 8 Jun', 'Mon 9 Jun'])
    const selected = wrapper.find('.btn-group').findAll('.btn-primary')
    expect(selected.map((b) => b.text())).toEqual(['Sat 7 Jun'])
  })

  it('does not slide when the screen is rotated', async () => {
    stubScreen(false)
    await mountSessions()
    await buttonIn(wrapper, 'Next Week').trigger('click')
    await flushPromises()
    expect(wrapper.find('transition-stub').attributes('name')).toBe('slide-forward')

    await resizeScreen(true)
    expect(wrapper.find('transition-stub').attributes('name')).toBe('none')
  })

  it('stays weekly in the list view', async () => {
    stubScreen(true)
    localStorage.setItem('view_mode', 'list')
    await mountSessions()

    expect(listSessions).toHaveBeenCalledWith(new Date('2025-06-02T00:00:00Z'), new Date('2025-06-09T00:00:00Z'))
    expect(pagerLabels()).toEqual(['This Week', 'Next Week', '16 – 22 Jun 2025', '23 – 29 Jun 2025'])

    // Switching to the calendar goes daily, on today
    await wrapper.find('button[title="Switch to Calendar View"]').trigger('click')
    await flushPromises()
    expect(listSessions).toHaveBeenLastCalledWith(new Date('2025-06-04T00:00:00Z'), new Date('2025-06-05T00:00:00Z'))
    expect(pagerLabels()).toEqual(['Today', 'Tomorrow', 'Fri 6 Jun'])
  })
})
