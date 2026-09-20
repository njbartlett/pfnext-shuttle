import { describe, expect, it } from 'vitest'
import { rankByScore } from './useRanking'

describe('rankByScore', () => {
  const entries = [
    { name: 'Ali', total: 12 },
    { name: 'Ben', total: 12 },
    { name: 'Cara', total: 9 },
    { name: 'Dev', total: 0 }
  ]

  it('groups ties under one rank with the first entry leading', () => {
    const rankings = rankByScore(entries, (e) => e.total)
    expect(rankings.map((r) => [r.rank, r.score, r.first.name, r.others.map((o) => o.name)])).toEqual([
      [1, 12, 'Ali', ['Ben']],
      [2, 9, 'Cara', []],
      [3, 0, 'Dev', []]
    ])
  })

  it('returns nothing for no entries', () => {
    expect(rankByScore([], () => 0)).toEqual([])
  })
})
