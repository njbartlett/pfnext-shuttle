// Session, booking and waitlist calls, including the credits opt-in
// handshake. Calls that act for a member take the member's id explicitly so
// that this module does not depend on either app's notion of "the current
// user" (the website lets admins act on behalf of other members).
import { apiRequest, ApiError, CREDITS_OPT_IN_REQUIRED } from './api'
import type { Booking, Session, UserRecord } from './types'

export type BookResult =
  | { outcome: 'booked' }
  | { outcome: 'credits_required'; cost: number }

export async function listSessions(from: Date, to: Date): Promise<Session[]> {
  return apiRequest<Session[]>('/sessions', {
    query: { from: from.toISOString(), to: to.toISOString() }
  })
}

export async function getSession(sessionId: number): Promise<Session> {
  return apiRequest<Session>(`/sessions/${sessionId}`)
}

export async function listBookings(personId: number, from: Date, to: Date): Promise<Booking[]> {
  return apiRequest<Booking[]>('/bookings', {
    query: {
      person_id: personId,
      from: from.toISOString(),
      to: to.toISOString()
    }
  })
}

export async function getUserRecord(personId: number): Promise<UserRecord> {
  return apiRequest<UserRecord>(`/users/${personId}`)
}

// Books a session. When the session costs credits and creditsUsed is 0, the
// server answers 402 credits_opt_in_required: the caller should confirm with
// the user and call again with creditsUsed = session cost.
export async function bookSession(
  personId: number,
  sessionId: number,
  creditsUsed: number,
  cost: number
): Promise<BookResult> {
  try {
    await apiRequest<void>('/bookings', {
      method: 'POST',
      body: {
        person_id: personId,
        session_id: sessionId,
        credits_used: creditsUsed
      }
    })
    return { outcome: 'booked' }
  } catch (e) {
    if (e instanceof ApiError && e.code === CREDITS_OPT_IN_REQUIRED) {
      return { outcome: 'credits_required', cost }
    }
    throw e
  }
}

export async function cancelBooking(personId: number, sessionId: number): Promise<void> {
  await apiRequest<unknown>('/bookings', {
    method: 'DELETE',
    query: { person_id: personId, session_id: sessionId }
  })
}

export async function joinWaitlist(personId: number, sessionId: number): Promise<void> {
  await apiRequest<unknown>('/waitlist', {
    method: 'POST',
    body: { person_id: personId, session_id: sessionId }
  })
}

export async function leaveWaitlist(personId: number, sessionId: number): Promise<void> {
  await apiRequest<void>('/waitlist', {
    method: 'DELETE',
    query: { person_id: personId, session_id: sessionId }
  })
}

export function isFull(session: Session): boolean {
  return session.max_booking_count !== null && session.booking_count >= session.max_booking_count
}

export function spacesLeft(session: Session): number | null {
  return session.max_booking_count === null
    ? null
    : Math.max(0, session.max_booking_count - session.booking_count)
}

export function isPastBookingDeadline(session: Session): boolean {
  return new Date(session.booking_deadline) < new Date()
}
