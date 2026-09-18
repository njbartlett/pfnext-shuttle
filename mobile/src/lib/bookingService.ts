// Mobile-side view of the shared booking service, with every member-scoped
// call bound to the logged-in user. Views import from here rather than from
// @pfnext/shared so they never need to pass a person id around.
import * as shared from '@pfnext/shared'
import type { Booking, BookResult, UserRecord } from '@pfnext/shared'
import { user } from './auth'

export { listSessions, getSession, isFull, spacesLeft, isPastBookingDeadline } from '@pfnext/shared'
export type { BookResult } from '@pfnext/shared'

function myId(): number {
  return user.value!.id
}

export function listMyBookings(from: Date, to: Date): Promise<Booking[]> {
  return shared.listBookings(myId(), from, to)
}

export function getMyUserRecord(): Promise<UserRecord> {
  return shared.getUserRecord(myId())
}

export function bookSession(sessionId: number, creditsUsed: number, cost: number): Promise<BookResult> {
  return shared.bookSession(myId(), sessionId, creditsUsed, cost)
}

export function cancelBooking(sessionId: number): Promise<void> {
  return shared.cancelBooking(myId(), sessionId)
}

export function joinWaitlist(sessionId: number): Promise<void> {
  return shared.joinWaitlist(myId(), sessionId)
}

export function leaveWaitlist(sessionId: number): Promise<void> {
  return shared.leaveWaitlist(myId(), sessionId)
}
