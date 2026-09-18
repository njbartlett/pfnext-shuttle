// Groups an already sorted (descending) list into ranks, with ties sharing
// a rank: used by the attendance stats and challenge leaderboards

export interface Ranking<T> {
  rank: number
  score: number
  first: T
  others: T[]
}

export function rankByScore<T>(entries: T[], scoreOf: (entry: T) => number): Ranking<T>[] {
  const rankings: Ranking<T>[] = []
  let current: Ranking<T> | null = null
  for (const entry of entries) {
    const score = scoreOf(entry)
    if (current && current.score === score) {
      current.others.push(entry)
    } else {
      current = { rank: rankings.length + 1, score, first: entry, others: [] }
      rankings.push(current)
    }
  }
  return rankings
}
