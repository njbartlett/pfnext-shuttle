// Response and request shapes of the pfnext API (see src/sessions.rs,
// src/bookings.rs, src/loginsession.rs, src/users.rs, src/polls.rs,
// src/activities.rs, src/transaction_log.rs in the backend).

export interface LoggedInUser {
  id: number
  name: string
  email: string
  roles: string[]
  // Bib colour ("green" | "red" | "blue") or null for members without one
  status?: string | null
  // Present only for bearer-token clients (mobile); the website uses a cookie
  token?: string
  expiry?: string
}

// Enough of a user to identify and display them; satisfied by LoggedInUser,
// UserRecord and the trainer/person fragments of other records
export interface UserSummary {
  id: number
  name: string
  email: string
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
  avg_rating: number | null
  count_rating_all: number
  count_rating_5: number
  count_rating_4: number
  count_rating_3: number
  count_rating_2: number
  count_rating_1: number
  max_booking_count: number | null
  attended_count: number | null
  notes: string | null
  cost: number
  booking_deadline_duration_mins: number
  booking_deadline: string
}

export interface NewSession {
  datetime: string
  duration_mins: number
  session_type_id: number
  location_id: number | null
  trainer_id: number | null
  max_bookings: number | null
  notes: string | null
  cost: number
  booking_deadline_mins: number
}

export interface Booking {
  person_id: number
  person_name: string
  person_email: string
  person_status: string | null
  session_id: number
  session_datetime: string
  session_duration_mins: number
  session_location: SessionLocation | null
  session_type: SessionType
  attended: boolean
  credits_used: number
  booked_timestamp: string | null
}

export interface Feedback {
  rating: number
  comment: string | null
}

export interface BookingUpdate {
  attended?: boolean
  feedback?: Feedback
}

export interface WaitlistEntry {
  id: number
  rank: number
  person_id: number
  person_name: string
  person_email: string
  person_status: string | null
  session_id: number
}

export interface AttendanceStat {
  person_id: number
  name: string
  email: string
  attended_count: number
}

export interface UserRecord {
  id: number
  name: string
  email: string
  phone: string | null
  emergency_name: string | null
  emergency_phone: string | null
  medical_info: string | null
  roles: string[]
  credits: number
  pwd_defined: boolean
  status: string | null
}

// Fields a member may change about themselves (PATCH /users/<id>)
export interface UserPatch {
  name?: string | null
  phone?: string | null
  emergency_name?: string | null
  emergency_phone?: string | null
  medical_info?: string | null
}

// Full admin update (PUT /users/<id>)
export interface UserUpdate {
  name: string
  email: string
  phone: string | null
  emergency_name: string | null
  emergency_phone: string | null
  medical_info: string | null
  roles: string[]
  credits: number
  status: string | null
}

export interface NewUserRequest {
  name: string
  email: string
  phone: string | null
  emergency_name: string | null
  emergency_phone: string | null
  medical_info: string | null
  // Used in the registration email
  website_url: string
  reset_url: string
}

export interface Poll {
  id: number
  question: string
  limit_per_person: number
  description: string | null
  open: boolean
}

export interface NewPoll {
  question: string
  limit_per_person: number
  description: string | null
  open: boolean
}

export interface Vote {
  id: number
  poll_id: number
  person_id: number
  person_name: string
  person_email: string
  value: string
}

export interface PollWithVotes {
  poll: Poll
  votes: Vote[]
}

export interface ActivityType {
  id: number
  name: string
  units: string
  step_size: number
}

export interface ChallengeRecord {
  id: number
  name: string
  description: string | null
  start: string
  finish: string
  activity_type: ActivityType
  goal: number | null
  individual_goal: number | null
  daily_goal: number | null
  total_all: number
  total_for_person: number
}

export interface MemberActivitySummary {
  id: number | null
  name: string | null
  total_amount: number
  singular_units: string
  plural_units: string
}

export interface DailyActivity {
  date: string
  amount: number
  completed: boolean
}

export interface ChallengeFull extends ChallengeRecord {
  daily_activities: DailyActivity[]
  member_summaries: MemberActivitySummary[]
}

export interface NewActivity {
  person_id: number
  challenge_id: number | null
  activity_type: number | null
  date: string
  amount: number
}

export interface Activity {
  id: number
  person_id: number
  person_name: string
  person_email: string
  challenge: ChallengeRecord | null
  activity_type: ActivityType
  date: string
  amount: number
}

export interface PostSummary {
  id: number
  title: string
  author_id: number
  author_name: string
  author_email: string
  created_at: string
  last_editor_id: number
  last_editor_name: string
  last_editor_email: string
  last_edited_at: string
  // null for an unpublished draft
  published_at: string | null
}

export interface Post extends PostSummary {
  // HTML, written by editors with TinyMCE
  content: string
}

export interface SavePost {
  title: string
  content: string
  published: string | null
}

export interface LogRow {
  id: number
  datetime: string
  person: string
  event_type: string
  detail: string
}

export interface LoginSessionRecord {
  uid: number
  sessionid: string
  name: string
  email: string
  roles: string[]
  loggedin: string | null
  loggedin_from: string | null
  expiry: string
  is_current: boolean
}
