// Session administration, attendance, feedback and waitlist listings
// (backend src/sessions.rs and src/bookings.rs). Member-facing booking
// calls live in bookingService.ts.
import { apiRequest } from './api'
import type {
  Booking, BookingUpdate, Feedback, NewSession, Session, SessionLocation, SessionType, WaitlistEntry
} from './types'

export interface SessionFilter {
  from?: Date
  to?: Date
  trainerId?: number
  attended?: boolean
}

export async function listSessionsFiltered(filter: SessionFilter): Promise<Session[]> {
  return apiRequest<Session[]>('/sessions', {
    query: {
      from: filter.from?.toISOString(),
      to: filter.to?.toISOString(),
      trainer_id: filter.trainerId,
      attended: filter.attended
    }
  })
}

// Resolves to the new session's id
export async function createSession(session: NewSession): Promise<number> {
  return apiRequest<number>('/sessions', { method: 'POST', body: session })
}

export async function updateSession(sessionId: number, session: NewSession): Promise<void> {
  await apiRequest<unknown>(`/sessions/${sessionId}`, { method: 'PUT', body: session })
}

export async function deleteSession(sessionId: number): Promise<void> {
  await apiRequest<void>(`/sessions/${sessionId}`, { method: 'DELETE' })
}

// Resolves to the ids of the sessions created
export async function createSessionsBatch(sessions: NewSession[]): Promise<number[]> {
  return apiRequest<number[]>('/sessions/batch', { method: 'POST', body: sessions })
}

export async function listLocations(): Promise<SessionLocation[]> {
  return apiRequest<SessionLocation[]>('/locations')
}

export async function listSessionTypes(deprecated?: boolean): Promise<SessionType[]> {
  return apiRequest<SessionType[]>('/session_types', { query: { deprecated } })
}

export async function listSessionBookings(sessionId: number): Promise<Booking[]> {
  return apiRequest<Booking[]>('/bookings', { query: { session_id: sessionId } })
}

// Trainer of the session or admin only
export async function listFeedback(sessionId: number): Promise<Feedback[]> {
  return apiRequest<Feedback[]>('/feedback', { query: { session_id: sessionId } })
}

export async function listWaitlist(sessionId: number): Promise<WaitlistEntry[]> {
  return apiRequest<WaitlistEntry[]>('/waitlist', { query: { session_id: sessionId } })
}

// Books a member onto a session without the credits handshake (admin use)
export async function addBooking(personId: number, sessionId: number): Promise<void> {
  await apiRequest<unknown>('/bookings', {
    method: 'POST',
    body: { person_id: personId, session_id: sessionId }
  })
}

// personId null applies the update to every booking of the session
export async function updateBooking(sessionId: number, personId: number | null, update: BookingUpdate): Promise<void> {
  await apiRequest<void>('/bookings', {
    method: 'PATCH',
    query: { session_id: sessionId, person_id: personId ?? undefined },
    body: update
  })
}

export async function setAttendance(sessionId: number, personId: number | null, attended: boolean): Promise<void> {
  await updateBooking(sessionId, personId, { attended })
}

export async function saveFeedback(sessionId: number, personId: number, feedback: Feedback): Promise<void> {
  await updateBooking(sessionId, personId, { feedback })
}
