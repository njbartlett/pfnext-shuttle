import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError, configureApi } from './api'
import { bookSession, isFull, isPastBookingDeadline, spacesLeft } from './bookingService'
import type { Session } from './types'

const fetchMock = vi.fn<typeof fetch>()

beforeEach(() => {
  vi.stubGlobal('fetch', fetchMock)
  configureApi({ baseUrl: '/api' })
})

afterEach(() => {
  fetchMock.mockReset()
  vi.unstubAllGlobals()
})

function session(overrides: Partial<Session>): Session {
  return { id: 1, booking_count: 0, max_booking_count: null, ...overrides } as Session
}

describe('bookSession credits handshake', () => {
  it('books directly when the server accepts', async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }))
    await expect(bookSession(3, 9, 0, 2)).resolves.toEqual({ outcome: 'booked' })
    expect(JSON.parse(String(fetchMock.mock.calls[0][1]?.body))).toEqual({ person_id: 3, session_id: 9, credits_used: 0 })
  })

  it('asks the caller to confirm when credits are required', async () => {
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ code: 'credits_opt_in_required', message: 'Costs 2 credits' }), {
        status: 402,
        headers: { 'Content-Type': 'application/json' }
      })
    )
    await expect(bookSession(3, 9, 0, 2)).resolves.toEqual({ outcome: 'credits_required', cost: 2 })
  })

  it('rethrows every other failure', async () => {
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ code: 'session_full', message: 'Full' }), {
        status: 409,
        headers: { 'Content-Type': 'application/json' }
      })
    )
    await expect(bookSession(3, 9, 0, 2)).rejects.toMatchObject<Partial<ApiError>>({ status: 409, code: 'session_full' })
  })
})

describe('capacity helpers', () => {
  it('treats sessions without a limit as never full', () => {
    const open = session({ booking_count: 50, max_booking_count: null })
    expect(isFull(open)).toBe(false)
    expect(spacesLeft(open)).toBeNull()
  })

  it('counts spaces down to zero', () => {
    expect(spacesLeft(session({ booking_count: 3, max_booking_count: 5 }))).toBe(2)
    expect(isFull(session({ booking_count: 5, max_booking_count: 5 }))).toBe(true)
    expect(spacesLeft(session({ booking_count: 7, max_booking_count: 5 }))).toBe(0)
  })

  it('compares the booking deadline with now', () => {
    expect(isPastBookingDeadline(session({ booking_deadline: new Date(Date.now() - 60000).toISOString() }))).toBe(true)
    expect(isPastBookingDeadline(session({ booking_deadline: new Date(Date.now() + 60000).toISOString() }))).toBe(false)
  })
})
