// Test data for component tests. The clock is pinned to a Wednesday in June
// 2025 with vi.setSystemTime(NOW) so that "this week", "past" and countdowns
// are deterministic; SESSION is the next morning.
import type { LoggedInUser, Session, SessionLocation, SessionType, UserRecord } from '@pfnext/shared'

export const NOW = new Date('2025-06-04T10:00:00Z')

// The same accounts as src/fixtures/users.sql
export const MEMBER: LoggedInUser = { id: 3, name: 'user1', email: 'user1@example.com', roles: [], status: null }
export const TRAINER: LoggedInUser = { id: 2, name: 'trainer', email: 'trainer@example.com', roles: ['trainer'], status: 'green' }
export const ADMIN: LoggedInUser = { id: 1, name: 'admin', email: 'admin@example.com', roles: ['admin'], status: null }

export const SESSION_TYPE: SessionType = { id: 1, name: 'HIIT', requires_trainer: true, cost: 2, deprecated: false }
export const LOCATION: SessionLocation = { id: 3, name: 'Oak Hill Park', address: 'Oak Hill Park, London', url: null }

export function makeUserRecord(user: LoggedInUser, overrides: Partial<UserRecord> = {}): UserRecord {
  return {
    id: user.id,
    name: user.name,
    email: user.email,
    phone: null,
    emergency_name: 'Next of Kin',
    emergency_phone: '01234 567890',
    medical_info: null,
    roles: user.roles,
    credits: 5,
    pwd_defined: true,
    status: user.status ?? null,
    ...overrides
  }
}

// A free, bookable session tomorrow morning with one booking
export function makeSession(overrides: Partial<Session> = {}): Session {
  return {
    id: 7,
    datetime: '2025-06-05T09:00:00Z',
    duration_mins: 60,
    session_type: SESSION_TYPE,
    location: LOCATION,
    trainer: { id: TRAINER.id, name: TRAINER.name, email: TRAINER.email, url: null },
    booked: false,
    waitlist_rank: null,
    attended: false,
    rating: null,
    comment: null,
    booking_count: 1,
    avg_rating: null,
    count_rating_all: 0,
    count_rating_5: 0,
    count_rating_4: 0,
    count_rating_3: 0,
    count_rating_2: 0,
    count_rating_1: 0,
    max_booking_count: null,
    attended_count: null,
    notes: 'Bring water',
    cost: 0,
    booking_deadline_duration_mins: 60,
    booking_deadline: '2025-06-05T08:00:00Z',
    ...overrides
  }
}
