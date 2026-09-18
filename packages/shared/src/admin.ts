// Admin-only listings: event log, login sessions, attendance statistics
// (backend src/transaction_log.rs, src/loginsession.rs, src/bookings.rs).
import { apiRequest } from './api'
import type { AttendanceStat, LoginSessionRecord, LogRow } from './types'

export async function readLog(from: Date, to: Date): Promise<LogRow[]> {
  return apiRequest<LogRow[]>('/log', { query: { from: from.toISOString(), to: to.toISOString() } })
}

export async function listLoginSessions(): Promise<LoginSessionRecord[]> {
  return apiRequest<LoginSessionRecord[]>('/loginsession')
}

export async function deleteLoginSession(sessionId: string): Promise<void> {
  await apiRequest<unknown>(`/loginsession/${encodeURIComponent(sessionId)}`, { method: 'DELETE' })
}

export interface AttendanceStatsFilter {
  from?: Date
  to?: Date
  sessionTypeIds?: number[]
}

export async function getAttendanceStats(filter: AttendanceStatsFilter): Promise<AttendanceStat[]> {
  return apiRequest<AttendanceStat[]>('/stats/attendance', {
    query: {
      from: filter.from?.toISOString(),
      to: filter.to?.toISOString(),
      session_type: filter.sessionTypeIds
    }
  })
}
