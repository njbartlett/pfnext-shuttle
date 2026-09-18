// Polls and votes (backend src/polls.rs).
import { apiRequest } from './api'
import type { NewPoll, Poll, PollWithVotes, Vote } from './types'

export interface PollFilter {
  // Restrict votes to this member; omit (admin only) to see everyone's votes
  personId?: number
  // Include closed polls
  closed?: boolean
}

export async function listPolls(filter: PollFilter = {}): Promise<PollWithVotes[]> {
  return apiRequest<PollWithVotes[]>('/polls', {
    query: { person_id: filter.personId, closed: filter.closed ? true : undefined }
  })
}

export async function createPoll(poll: NewPoll): Promise<Poll> {
  return apiRequest<Poll>('/polls', { method: 'POST', body: poll })
}

export async function updatePoll(pollId: number, poll: NewPoll): Promise<Poll> {
  return apiRequest<Poll>(`/polls/${pollId}`, { method: 'PUT', body: poll })
}

export async function deletePoll(pollId: number): Promise<void> {
  await apiRequest<unknown>(`/polls/${pollId}`, { method: 'DELETE' })
}

export async function submitVote(personId: number, pollId: number, value: string): Promise<Vote> {
  return apiRequest<Vote>('/votes', {
    method: 'POST',
    body: { person_id: personId, poll_id: pollId, value }
  })
}

export async function deleteVote(voteId: number): Promise<void> {
  await apiRequest<unknown>(`/votes/${voteId}`, { method: 'DELETE' })
}
