// The sessions page with a logged-in member: the alerts, the booking flow
// (including the credits confirmation), cancelling, the waitlist and the
// ?week= parameter. API calls are mocked; see testing/sharedMock.ts.
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
})
