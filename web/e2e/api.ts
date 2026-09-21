// Direct API access for arranging test data (creating the session a spec
// books, for instance) and for checking what the UI did against the server.
import { expect, type APIRequestContext, type PlaywrightWorkerArgs } from '@playwright/test'
import { PASSWORD, type Account } from './fixtures'

type Playwright = PlaywrightWorkerArgs['playwright']

interface Named {
  id: number
  name: string
}

export interface SessionRequest {
  datetime: Date
  notes: string
  trainerEmail?: string
  cost?: number
  maxBookings?: number | null
}

export interface SessionSummary {
  id: number
  notes: string | null
  trainer: Named | null
}

export interface BookingSummary {
  person_id: number
  person_name: string
  attended: boolean
}

// A request context logged in as `account`, from no cookies at all so the
// browser state used by the page tests cannot leak in
export async function loginApi(playwright: Playwright, baseURL: string | undefined, account: Account): Promise<APIRequestContext> {
  const api = await playwright.request.newContext({ baseURL, storageState: { cookies: [], origins: [] } })
  const login = await api.post('/api/login', { data: { email: account.email, password: PASSWORD } })
  expect(login.ok(), `login as ${account.name}`).toBe(true)
  return api
}

export async function findUser(admin: APIRequestContext, email: string): Promise<Named> {
  const [user] = (await (await admin.get(`/api/users/list?email=${encodeURIComponent(email)}`)).json()) as Named[]
  expect(user, `user ${email}`).toBeDefined()
  return user
}

// Creates a session as the admin; resolves to its id
export async function createSession(admin: APIRequestContext, request: SessionRequest): Promise<number> {
  const [sessionType] = (await (await admin.get('/api/session_types?deprecated=false')).json()) as Named[]
  const [location] = (await (await admin.get('/api/locations')).json()) as Named[]
  const trainer = request.trainerEmail ? await findUser(admin, request.trainerEmail) : ((await (await admin.get('/api/users/list?role=trainer')).json()) as Named[])[0]

  const created = await admin.post('/api/sessions', {
    data: {
      datetime: request.datetime.toISOString(),
      duration_mins: 60,
      session_type_id: sessionType.id,
      location_id: location.id,
      trainer_id: trainer.id,
      max_bookings: request.maxBookings ?? null,
      notes: request.notes,
      cost: request.cost ?? 0,
      booking_deadline_mins: 0
    }
  })
  expect(created.ok(), `create session: ${await created.text()}`).toBe(true)
  return (await created.json()) as number
}

export async function deleteSession(admin: APIRequestContext, sessionId: number): Promise<void> {
  await admin.delete(`/api/sessions/${sessionId}`)
}

// Sessions in the week of `date`, for cleaning up what a spec created
export async function listSessionsInWeek(admin: APIRequestContext, date: Date): Promise<SessionSummary[]> {
  const from = new Date(date)
  from.setDate(from.getDate() - 7)
  const to = new Date(date)
  to.setDate(to.getDate() + 7)
  return (await (await admin.get(`/api/sessions?from=${from.toISOString()}&to=${to.toISOString()}`)).json()) as SessionSummary[]
}

// Books a member onto a session as the admin (no credits handshake)
export async function bookMember(admin: APIRequestContext, personId: number, sessionId: number): Promise<void> {
  const booked = await admin.post('/api/bookings', { data: { person_id: personId, session_id: sessionId } })
  expect(booked.ok(), `book member ${personId}: ${await booked.text()}`).toBe(true)
}

export async function listSessionBookings(api: APIRequestContext, sessionId: number): Promise<BookingSummary[]> {
  return (await (await api.get(`/api/bookings?session_id=${sessionId}`)).json()) as BookingSummary[]
}
