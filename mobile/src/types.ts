// Response shapes of the pfnext /api/v1 endpoints (see src/sessions.rs,
// src/bookings.rs, src/loginsession.rs, src/users.rs in the backend).

export interface LoggedInUser {
  id: number
  name: string
  email: string
  roles: string[]
  token?: string
  expiry?: string
}

export interface SessionType {
  id: number
  name: string
  requires_trainer: boolean
  cost: number
  deprecated: boolean
}

export interface SessionLocation {
  id: number
  name: string
  address: string
  url: string | null
}

export interface SessionTrainer {
  id: number
  name: string
  email: string
  url: string | null
}

export interface Session {
  id: number
  datetime: string
  duration_mins: number
  session_type: SessionType
  location: SessionLocation | null
  trainer: SessionTrainer | null
  booked: boolean
  waitlist_rank: number | null
  attended: boolean
  rating: number | null
  comment: string | null
  booking_count: number
  max_booking_count: number | null
  notes: string | null
  cost: number
  booking_deadline_duration_mins: number
  booking_deadline: string
}

export interface Booking {
  person_id: number
  person_name: string
  person_email: string
  session_id: number
  session_datetime: string
  session_duration_mins: number
  session_location: SessionLocation | null
  session_type: SessionType
  attended: boolean
  credits_used: number
  booked_timestamp: string | null
}

export interface UserRecord {
  id: number
  name: string
  email: string
  phone: string | null
  credits: number
  roles: string[] | string
}
