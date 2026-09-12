// Booking, cancellation and waitlist calls, including the credits opt-in
// handshake shared by the sessions and bookings pages.
import { apiRequest, ApiError, CREDITS_OPT_IN_REQUIRED } from './api'
import { user } from './auth'
import type { Booking, Session, UserRecord } from '@/types'

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

export async function listMyBookings(from: Date, to: Date): Promise<Booking[]> {
  return apiRequest<Booking[]>('/bookings', {
    query: {
      person_id: user.value!.id,
      from: from.toISOString(),
      to: to.toISOString()
    }
  })
}

export async function getMyUserRecord(): Promise<UserRecord> {
  return apiRequest<UserRecord>(`/users/${user.value!.id}`)
}

// Books a session. When the session costs credits and creditsUsed is 0, the
// server answers 402 credits_opt_in_required: the caller should confirm with
// the user and call again with creditsUsed = session cost.
export async function bookSession(sessionId: number, creditsUsed: number, cost: number): Promise<BookResult> {
  try {
    await apiRequest<void>('/bookings', {
      method: 'POST',
      body: {
        person_id: user.value!.id,
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

export async function cancelBooking(sessionId: number): Promise<void> {
  await apiRequest<unknown>('/bookings', {
    method: 'DELETE',
    query: { person_id: user.value!.id, session_id: sessionId }
  })
}

export async function joinWaitlist(sessionId: number): Promise<void> {
  await apiRequest<unknown>('/waitlist', {
    method: 'POST',
    body: { person_id: user.value!.id, session_id: sessionId }
  })
}

export async function leaveWaitlist(sessionId: number): Promise<void> {
  await apiRequest<void>('/waitlist', {
    method: 'DELETE',
    query: { person_id: user.value!.id, session_id: sessionId }
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
