// The attendance page's waitlist: shown only when a limited session is full,
// and letting the admin or session trainer add other members. API calls are mocked.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, type VueWrapper } from '@vue/test-utils'
import {
  getSession, joinWaitlist, listSessionBookings, listUsers, listWaitlist,
  type Booking, type WaitlistEntry
} from '@pfnext/shared'
import AttendanceView from './AttendanceView.vue'
import { apiError } from '@/stores/apiError'
import { setUser } from '@/stores/auth'
import { ADMIN, FULL_MEMBER, LIMITED_MEMBER, makeSession, makeUserRecord, MEMBER, SESSION_TYPE, TRAINER } from '@/testing/fixtures'
import { mountAt } from '@/testing/mount'

vi.mock('@pfnext/shared', () => import('@/testing/sharedMock').then((m) => m.mockedShared()))

function makeBooking(person: { id: number; name: string; email: string }): Booking {
  return {
    person_id: person.id,
    person_name: person.name,
    person_email: person.email,
    person_status: null,
    session_id: 7,
    session_datetime: '2025-06-05T09:00:00Z',
    session_duration_mins: 60,
    session_location: null,
    session_type: SESSION_TYPE,
    attended: false,
    credits_used: 0,
    booked_timestamp: '2025-06-01T09:00:00Z'
  }
}

function makeWaitlistEntry(person: { id: number; name: string; email: string }, rank: number): WaitlistEntry {
  return { id: rank, rank, person_id: person.id, person_name: person.name, person_email: person.email, person_status: null, session_id: 7 }
}

let wrapper: VueWrapper

async function mountAttendance() {
  ;({ wrapper } = await mountAt(AttendanceView, '/attendance.html?id=7'))
}

function waitlistSection() {
  return wrapper.findAll('li.list-group-item').find((li) => li.find('h2').text() === 'Waitlist')
}

function waitlistOptions(): string[] {
  return waitlistSection()!.findAll('datalist option').map((o) => o.attributes('value')!)
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(listUsers).mockResolvedValue([ADMIN, TRAINER, MEMBER, FULL_MEMBER, LIMITED_MEMBER].map((u) => makeUserRecord(u)))
  vi.mocked(listSessionBookings).mockResolvedValue([makeBooking(MEMBER)])
  vi.mocked(listWaitlist).mockResolvedValue([])
  setUser(TRAINER)
})

afterEach(() => {
  wrapper?.unmount()
  setUser(null)
  expect(apiError.value).toBeNull()
})

describe('AttendanceView waitlist', () => {
  it('is hidden when the session has no booking limit', async () => {
    vi.mocked(getSession).mockResolvedValue(makeSession({ max_booking_count: null }))
    await mountAttendance()

    expect(waitlistSection()).toBeUndefined()
  })

  it('is hidden when the booking limit has not been reached', async () => {
    vi.mocked(getSession).mockResolvedValue(makeSession({ max_booking_count: 2 }))
    await mountAttendance()

    expect(waitlistSection()).toBeUndefined()
  })

  it('is shown when the booking limit has been reached, even when empty', async () => {
    vi.mocked(getSession).mockResolvedValue(makeSession({ max_booking_count: 1 }))
    await mountAttendance()

    expect(waitlistSection()!.text()).toContain('nobody is on the waitlist')
  })

  it('offers only members who are not booked, waiting or training', async () => {
    vi.mocked(getSession).mockResolvedValue(makeSession({ max_booking_count: 1 }))
    vi.mocked(listWaitlist).mockResolvedValue([makeWaitlistEntry(FULL_MEMBER, 1)])
    await mountAttendance()

    expect(waitlistOptions()).toEqual(['admin <admin@example.com>', 'limited <limited@example.com>'])
  })

  it('adds the picked member to the waitlist', async () => {
    vi.mocked(getSession).mockResolvedValue(makeSession({ max_booking_count: 1 }))
    vi.mocked(joinWaitlist).mockResolvedValue(makeWaitlistEntry(LIMITED_MEMBER, 1))
    await mountAttendance()

    const section = waitlistSection()!
    await section.find('input[type=search]').setValue('limited <limited@example.com>')
    vi.mocked(listWaitlist).mockResolvedValue([makeWaitlistEntry(LIMITED_MEMBER, 1)])
    await section.find('form').trigger('submit')
    await flushPromises()

    expect(joinWaitlist).toHaveBeenCalledWith(LIMITED_MEMBER.id, 7)
    expect(waitlistSection()!.find('tbody').text()).toContain('limited')
    expect((waitlistSection()!.find('input[type=search]').element as HTMLInputElement).value).toBe('')
  })
})
