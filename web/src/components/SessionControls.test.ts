import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { VueWrapper } from '@vue/test-utils'
import type { Session } from '@pfnext/shared'
import SessionControls from './SessionControls.vue'
import { setUser } from '@/stores/auth'
import { ADMIN, makeSession, MEMBER, NOW, TRAINER } from '@/testing/fixtures'
import { mountAt } from '@/testing/mount'

// The page the controls sit on; the admin tools link back to it
const PAGE = '/sessions.html?week=2025-06-02'

let wrapper: VueWrapper

async function mountControls(session: Session) {
  wrapper = (await mountAt(SessionControls, PAGE, { props: { session, now: NOW } })).wrapper
}

function buttonLabels(): string[] {
  return wrapper.findAll('button').map((button) => button.text())
}

afterEach(() => {
  wrapper.unmount()
  setUser(null)
})

describe('SessionControls logged out', () => {
  it('renders nothing', async () => {
    setUser(null)
    await mountControls(makeSession())
    expect(wrapper.text()).toBe('')
    expect(wrapper.find('button').exists()).toBe(false)
  })
})

describe('SessionControls for a member', () => {
  beforeEach(() => setUser(MEMBER))

  it('offers to book a future session with a distant deadline', async () => {
    const session = makeSession()
    await mountControls(session)
    expect(buttonLabels()).toEqual(['Book'])

    await wrapper.find('button').trigger('click')
    expect(wrapper.emitted('book')).toEqual([[session]])
  })

  it('shows the time left when the deadline is within three hours', async () => {
    await mountControls(
      makeSession({ datetime: '2025-06-04T13:00:00Z', booking_deadline: '2025-06-04T12:00:00Z' })
    )
    expect(wrapper.find('button').text()).toMatch(/^Book\swithin 2h 0m$/)
  })

  it('refuses once the deadline has passed', async () => {
    await mountControls(
      makeSession({ datetime: '2025-06-04T11:00:00Z', booking_deadline: '2025-06-04T09:00:00Z' })
    )
    expect(wrapper.text()).toMatch(/Deadline Passed!/)
    expect(buttonLabels()).toEqual([])
  })

  it('offers the waitlist when the session is full', async () => {
    const session = makeSession({ max_booking_count: 1, booking_count: 1 })
    await mountControls(session)
    expect(buttonLabels()).toEqual(['Join Waitlist'])

    await wrapper.find('button').trigger('click')
    expect(wrapper.emitted('join-waitlist')).toEqual([[session]])
  })

  it('shows the waitlist position and lets the member leave', async () => {
    const session = makeSession({ max_booking_count: 1, booking_count: 1, waitlist_rank: 2 })
    await mountControls(session)
    expect(wrapper.find('button').text()).toMatch(/On Waitlist\s\(2\)/)

    await wrapper.find('a.dropdown-item').trigger('click')
    expect(wrapper.emitted('leave-waitlist')).toEqual([[session]])
  })

  it('shows a booked future session with a cancel option', async () => {
    const session = makeSession({ booked: true, booking_count: 2 })
    await mountControls(session)
    expect(buttonLabels()).toEqual(['Booked!'])

    const cancel = wrapper.find('a.dropdown-item')
    expect(cancel.text()).toBe('Cancel')
    await cancel.trigger('click')
    expect(wrapper.emitted('cancel')).toEqual([[session]])
  })

  it('shows attendance and a feedback button for an attended past session', async () => {
    const session = makeSession({
      datetime: '2025-06-03T09:00:00Z',
      booking_deadline: '2025-06-03T08:00:00Z',
      booked: true,
      attended: true
    })
    await mountControls(session)
    expect(wrapper.text()).toMatch(/Attended/)
    expect(wrapper.text()).not.toMatch(/Didn't Attend/)

    await wrapper.find('button[title="Give feedback"]').trigger('click')
    expect(wrapper.emitted('feedback')).toEqual([[session]])
  })

  it('shows a missed past session without any action', async () => {
    await mountControls(
      makeSession({
        datetime: '2025-06-03T09:00:00Z',
        booking_deadline: '2025-06-03T08:00:00Z',
        booked: true,
        attended: false
      })
    )
    expect(wrapper.text()).toMatch(/Didn't Attend/)
    expect(buttonLabels()).toEqual([])
  })

  it('shows nothing for a past session the member did not book', async () => {
    await mountControls(makeSession({ datetime: '2025-06-03T09:00:00Z', booking_deadline: '2025-06-03T08:00:00Z' }))
    expect(wrapper.text()).toBe('')
  })
})

describe('SessionControls for staff', () => {
  it('gives admins the tools menu, linking back to this page', async () => {
    setUser(ADMIN)
    const session = makeSession()
    await mountControls(session)

    const links = wrapper.findAll('a.dropdown-item')
    expect(links.map((link) => link.text())).toEqual(['Attendance', 'Feedback', 'Edit', 'Copy'])
    expect(links.map((link) => link.attributes('href'))).toEqual([
      `/attendance.html?id=7&return=${PAGE}`,
      `/feedback.html?id=7&return=${PAGE}`,
      `/edit_session.html?edit=7&return=${PAGE}`,
      `/edit_session.html?copy=7&return=${PAGE}`
    ])

    await wrapper.find('button.dropdown-item').trigger('click')
    expect(wrapper.emitted('delete')).toEqual([[session]])
  })

  it('gives the session trainer only the attendance link', async () => {
    setUser(TRAINER)
    await mountControls(makeSession())

    expect(wrapper.find('.dropup').exists()).toBe(false)
    const attendance = wrapper.find('a.btn')
    expect(attendance.text()).toBe('Attendance')
    expect(attendance.attributes('href')).toBe(`/attendance.html?id=7&return=${PAGE}`)
  })

  it("gives a trainer nothing extra for another trainer's session", async () => {
    setUser(TRAINER)
    await mountControls(makeSession({ trainer: { id: 99, name: 'someone', email: 'x@example.com', url: null } }))

    expect(wrapper.find('a.btn').exists()).toBe(false)
    expect(buttonLabels()).toEqual(['Book'])
  })
})
