// Activities and challenges (backend src/activities.rs). Dates are calendar
// days as YYYY-MM-DD strings.
import { apiRequest } from './api'
import type { Activity, ActivityType, ChallengeFull, NewActivity } from './types'

export interface ActivityFilter {
  personId?: number
  challengeId?: number
  activityTypeId?: number
  from?: string
  to?: string
}

export async function listActivityTypes(): Promise<ActivityType[]> {
  return apiRequest<ActivityType[]>('/activity_types')
}

export async function listActivities(filter: ActivityFilter): Promise<Activity[]> {
  return apiRequest<Activity[]>('/activities', {
    query: {
      person_id: filter.personId,
      challenge_id: filter.challengeId,
      activity_type: filter.activityTypeId,
      from: filter.from,
      to: filter.to
    }
  })
}

export async function createActivity(activity: NewActivity): Promise<void> {
  await apiRequest<unknown>('/activities', { method: 'POST', body: activity })
}

export async function deleteActivity(activityId: number): Promise<void> {
  await apiRequest<void>(`/activities/${activityId}`, { method: 'DELETE' })
}

// dateToday lets admins preview the challenges as of another day
export async function listChallenges(personId: number, leaderboardLimit: number, dateToday?: string): Promise<ChallengeFull[]> {
  return apiRequest<ChallengeFull[]>('/challenges', {
    query: { person_id: personId, leaderboard_limit: leaderboardLimit, date_today: dateToday }
  })
}

export async function getChallenge(
  challengeId: number,
  personId: number,
  leaderboardLimit: number,
  dateNow?: string
): Promise<ChallengeFull> {
  return apiRequest<ChallengeFull>(`/challenges/${challengeId}`, {
    query: { person_id: personId, leaderboard_limit: leaderboardLimit, date_now: dateNow }
  })
}
