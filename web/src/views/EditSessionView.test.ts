// The session editor: filling the form from ?edit= and ?copy=, saving,
// validation and the return link. API calls are mocked.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, type VueWrapper } from '@vue/test-utils'
import type { Router } from 'vue-router'
import {
  createSession, getSession, listLocations, listSessionTypes, listUsers, updateSession, type NewSession
} from '@pfnext/shared'
import EditSessionView from './EditSessionView.vue'
import { apiError } from '@/stores/apiError'
import { setUser } from '@/stores/auth'
import { ADMIN, LOCATION, makeSession, makeUserRecord, SESSION_TYPE, TRAINER } from '@/testing/fixtures'
import { mountAt, waitFor } from '@/testing/mount'

vi.mock('@pfnext/shared', () => import('@/testing/sharedMock').then((m) => m.mockedShared()))

const NO_TRAINER_TYPE = { id: 2, name: 'On The Move', requires_trainer: false, cost: 0, deprecated: false }

// Session 7 as the API returns it: 45 minutes, limited to 10, 1h30 deadline
const STORED = makeSession({
  datetime: '2025-06-05T09:30:00Z',
  duration_mins: 45,
  max_booking_count: 10,
  cost: 2,
  booking_deadline_duration_mins: 90
})

// The same session as the editor sends it back
const REQUEST: NewSession = {
  datetime: '2025-06-05T09:30:00.000Z',
  duration_mins: 45,
  session_type_id: SESSION_TYPE.id,
  location_id: LOCATION.id,
  trainer_id: TRAINER.id,
  cost: 2,
  max_bookings: 10,
  notes: 'Bring water',
  booking_deadline_mins: 90
}

let wrapper: VueWrapper
let router: Router

async function mountEditor(path: string) {
  ;({ wrapper, router } = await mountAt(EditSessionView, path))
}

function value(selector: string): string {
  return (wrapper.find(selector).element as HTMLInputElement | HTMLSelectElement).value
}

function heading(): string {
  return wrapper.find('h2').text()
}

function formValues() {
  return {
    date: value('#editSessionDate'),
    time: value('#editSessionTime'),
    duration: value('#editSessionDuration'),
    type: value('#editSessionType'),
    cost: value('#editSessionCost'),
    limitEnabled: (wrapper.find('input[type=checkbox]').element as HTMLInputElement).checked,
    limit: value('#editSessionBookingLimit'),
    trainer: value('#editSessionTrainer'),
    location: value('#editSessionLocation'),
    deadlineHours: value('#editSessionBookingDeadlineHours'),
    deadlineMins: value('#editSessionBookingDeadlineMins'),
    notes: value('#editSessionNotes')
  }
}

const STORED_FORM = {
  date: '2025-06-05',
  time: '09:30',
  duration: '45',
  type: '1',
  cost: '2',
  limitEnabled: true,
  limit: '10',
  trainer: '2',
  location: '3',
  deadlineHours: '1',
  deadlineMins: '30',
  notes: 'Bring water'
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(listSessionTypes).mockResolvedValue([SESSION_TYPE, NO_TRAINER_TYPE])
  vi.mocked(listUsers).mockResolvedValue([makeUserRecord(TRAINER)])
  vi.mocked(listLocations).mockResolvedValue([LOCATION])
  vi.mocked(getSession).mockResolvedValue(STORED)
  setUser(ADMIN)
})

afterEach(() => {
  wrapper?.unmount()
  setUser(null)
  expect(apiError.value).toBeNull()
})

describe('EditSessionView opening', () => {
  it('starts empty with saving disabled', async () => {
    await mountEditor('/edit_session.html')

    expect(heading()).toBe('Create New Session')
    expect(getSession).not.toHaveBeenCalled()
    expect(listSessionTypes).toHaveBeenCalledWith(false)
    expect(listUsers).toHaveBeenCalledWith({ role: 'trainer' })
    expect(wrapper.find('#editSessionSubmit').attributes('disabled')).toBeDefined()
    expect(wrapper.find('#editSessionType').findAll('option').map((o) => o.text())).toEqual(['Type…', 'HIIT', 'On The Move'])
  })

  it('fills the form from ?edit=', async () => {
    await mountEditor('/edit_session.html?edit=7')

    expect(getSession).toHaveBeenCalledWith(7)
    expect(heading()).toBe('Edit Session')
    expect(formValues()).toEqual(STORED_FORM)
    expect(wrapper.text()).toContain('Deadline to book is 05/06/2025, 08:00')
    expect(wrapper.find('#editSessionSubmit').attributes('disabled')).toBeUndefined()
  })

  it('fills the form from ?copy= as a new session', async () => {
    await mountEditor('/edit_session.html?copy=7')

    expect(getSession).toHaveBeenCalledWith(7)
    expect(heading()).toBe('Create New Session')
    expect(formValues()).toEqual(STORED_FORM)
  })
})

describe('EditSessionView saving', () => {
  it('updates the edited session and confirms', async () => {
    vi.mocked(updateSession).mockResolvedValue(undefined)
    await mountEditor('/edit_session.html?edit=7')

    await wrapper.find('#editSessionNotes').setValue('Bring water and a mat')
    await wrapper.find('#editSessionSubmit').trigger('click')
    await flushPromises()

    expect(updateSession).toHaveBeenCalledWith(7, { ...REQUEST, notes: 'Bring water and a mat' })
    expect(createSession).not.toHaveBeenCalled()
    expect(wrapper.text()).toContain('Saved!')
    expect(router.currentRoute.value.query).toEqual({ edit: '7' })
  })

  it('creates a copy and switches to editing the new session', async () => {
    vi.mocked(createSession).mockResolvedValue(42)
    await mountEditor('/edit_session.html?copy=7')

    await wrapper.find('#editSessionSubmit').trigger('click')
    await flushPromises()

    expect(createSession).toHaveBeenCalledWith(REQUEST)
    expect(updateSession).not.toHaveBeenCalled()
    expect(getSession).toHaveBeenLastCalledWith(42)
    expect(router.currentRoute.value.query).toEqual({ edit: '42' })
    expect(heading()).toBe('Edit Session')
  })

  it('saves and keeps the form as the template for another', async () => {
    vi.mocked(updateSession).mockResolvedValue(undefined)
    await mountEditor('/edit_session.html?edit=7')

    await wrapper.find('#editSessionSubmitAndCreate').trigger('click')
    await flushPromises()

    expect(updateSession).toHaveBeenCalledWith(7, REQUEST)
    expect(router.currentRoute.value.query).toEqual({ copy: '7' })
    expect(heading()).toBe('Create New Session')
    expect(formValues()).toEqual(STORED_FORM)
  })
})

describe('EditSessionView validation', () => {
  it('requires a trainer for session types that need one', async () => {
    await mountEditor('/edit_session.html?copy=7')

    // setValue() on an <option> selects it (the "None" option has a null value)
    const trainerOptions = wrapper.find('#editSessionTrainer').findAll('option')
    await trainerOptions[trainerOptions.length - 1].setValue()
    expect(wrapper.text()).toContain("Sessions of type 'HIIT' require a trainer to be selected.")
    expect(wrapper.find('#editSessionSubmit').attributes('disabled')).toBeDefined()

    // A type without that requirement clears the error and brings its own cost
    await wrapper.find('#editSessionType').setValue(String(NO_TRAINER_TYPE.id))
    expect(wrapper.text()).not.toContain('require a trainer')
    expect(value('#editSessionCost')).toBe('0')
    expect(wrapper.find('#editSessionSubmit').attributes('disabled')).toBeUndefined()
  })
})

describe('EditSessionView closing', () => {
  it('returns to the ?return= page', async () => {
    await mountEditor('/edit_session.html?edit=7&return=/sessions.html?week=2025-06-02')

    await wrapper.find('.btn-close').trigger('click')
    // The sessions page is lazy-loaded, so the navigation takes longer than a tick
    await waitFor(() => expect(router.currentRoute.value.fullPath).toBe('/sessions.html?week=2025-06-02'))
  })

  it('falls back to the sessions page', async () => {
    await mountEditor('/edit_session.html?edit=7')

    await wrapper.find('.btn-close').trigger('click')
    await waitFor(() => expect(router.currentRoute.value.fullPath).toBe('/sessions.html'))
  })
})
