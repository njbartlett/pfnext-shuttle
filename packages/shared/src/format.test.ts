import { describe, expect, it } from 'vitest'
import {
  addMonths, displayDateRange, displayDateTime, displayFullDate, displayRelativeDay, displayTime, displayVenueDate, displayVenueTime, endOfMonth,
  formatDuration, groupByDay, sortByField, startOfDay, startOfMonth, startOfWeek
} from './format'

// Tests run with TZ=UTC (vitest.config.ts), so the browser-zone helpers show
// UTC while the venue helpers must still show Europe/London

describe('venue-zone display', () => {
  it('shows the venue time during British Summer Time', () => {
    expect(displayVenueTime('2025-07-01T17:30:00Z')).toBe('18:30')
    expect(displayTime('2025-07-01T17:30:00Z')).toBe('17:30')
  })

  it('shows the venue time in winter', () => {
    expect(displayVenueTime('2025-01-06T18:30:00Z')).toBe('18:30')
  })

  it('formats full dates and date-times', () => {
    expect(displayFullDate('2025-01-06T18:30:00Z')).toBe('Mon, 6 January 2025')
    expect(displayDateTime('2025-01-06T18:30:00Z')).toBe('6 Jan 2025 18:30:00')
    expect(displayDateTime(null)).toBeNull()
  })

  it('rolls a late evening over to the next venue day in summer', () => {
    expect(displayFullDate('2025-07-01T23:30:00Z')).toBe('Wed, 2 July 2025')
    expect(displayVenueDate('2025-07-01T23:30:00Z')).toBe('Wed 2 Jul')
  })

  it('abbreviates the venue date without the year', () => {
    expect(displayVenueDate('2025-01-06T18:30:00Z')).toBe('Mon 6 Jan')
  })

  it('names yesterday, today and tomorrow as venue days', () => {
    const now = new Date('2025-01-06T12:00:00Z')
    expect(displayRelativeDay('2025-01-05T18:30:00Z', now)).toBe('Yesterday')
    expect(displayRelativeDay('2025-01-06T18:30:00Z', now)).toBe('Today')
    expect(displayRelativeDay('2025-01-07T18:30:00Z', now)).toBe('Tomorrow')
    expect(displayRelativeDay('2025-01-08T18:30:00Z', now)).toBeNull()
    expect(displayRelativeDay('2025-01-04T18:30:00Z', now)).toBeNull()
  })

  it('judges the day boundary in the venue zone, not UTC', () => {
    // 23:30 UTC on 1 July is 00:30 BST on 2 July: tomorrow relative to a
    // 1 July afternoon, not today
    const now = new Date('2025-07-01T12:00:00Z')
    expect(displayRelativeDay('2025-07-01T23:30:00Z', now)).toBe('Tomorrow')
    expect(displayRelativeDay('2025-07-01T22:30:00Z', now)).toBe('Today')
  })
})

describe('displayDateRange', () => {
  it('omits the month and year when they are shared', () => {
    expect(displayDateRange(new Date(2025, 0, 6), new Date(2025, 0, 12))).toBe('6 – 12 Jan 2025')
  })

  it('repeats the month when it differs', () => {
    expect(displayDateRange(new Date(2025, 0, 27), new Date(2025, 1, 2))).toBe('27 Jan – 2 Feb 2025')
  })

  it('repeats the year when it differs', () => {
    expect(displayDateRange(new Date(2024, 11, 30), new Date(2025, 0, 5))).toBe('30 Dec 2024 – 5 Jan 2025')
  })
})

describe('formatDuration', () => {
  it('picks the two most significant units', () => {
    expect(formatDuration(2 * 86400000 + 3 * 3600000)).toBe('~ 2d 3h')
    expect(formatDuration(3 * 3600000 + 12 * 60000)).toBe('3h 12m')
    expect(formatDuration(12 * 60000 + 5000)).toBe('12m 5s')
    expect(formatDuration(5000)).toBe('5s')
  })

  it('ignores the sign', () => {
    expect(formatDuration(-90000)).toBe('1m 30s')
  })
})

describe('calendar arithmetic', () => {
  it('starts weeks on Monday', () => {
    const monday = new Date(2025, 0, 6)
    expect(startOfWeek(new Date(2025, 0, 8, 15, 45))).toEqual(monday) // Wednesday
    expect(startOfWeek(new Date(2025, 0, 12, 23, 59))).toEqual(monday) // Sunday
    expect(startOfWeek(new Date(2025, 0, 6, 9))).toEqual(monday) // Monday itself
  })

  it('starts days at local midnight', () => {
    expect(startOfDay(new Date(2025, 0, 8, 15, 45, 30, 500))).toEqual(new Date(2025, 0, 8))
  })

  it('bounds months', () => {
    expect(startOfMonth(new Date(2025, 1, 10))).toEqual(new Date(2025, 1, 1))
    expect(endOfMonth(new Date(2025, 1, 10))).toEqual(new Date(2025, 1, 28, 23, 59, 59, 999))
    expect(endOfMonth(new Date(2024, 1, 10))).toEqual(new Date(2024, 1, 29, 23, 59, 59, 999))
  })

  it('adds months across a year boundary', () => {
    expect(addMonths(new Date(2024, 10, 15), 3)).toEqual(new Date(2025, 1, 15))
  })
})

describe('sortByField', () => {
  const rows = () => [
    { name: 'Cara', rating: 4 },
    { name: 'Ali', rating: null },
    { name: 'Ben', rating: 5 }
  ]

  it('sorts strings and numbers, nulls first ascending', () => {
    expect(sortByField(rows(), 'name').map((r) => r.name)).toEqual(['Ali', 'Ben', 'Cara'])
    expect(sortByField(rows(), 'rating').map((r) => r.name)).toEqual(['Ali', 'Cara', 'Ben'])
  })

  it('puts nulls last descending', () => {
    expect(sortByField(rows(), 'rating', false).map((r) => r.name)).toEqual(['Ben', 'Cara', 'Ali'])
  })
})

describe('groupByDay', () => {
  it('groups by calendar day in first-seen order', () => {
    const items = [
      { at: '2025-01-06T09:00:00Z', id: 1 },
      { at: '2025-01-07T09:00:00Z', id: 2 },
      { at: '2025-01-06T18:00:00Z', id: 3 }
    ]
    const groups = groupByDay(items, (item) => item.at)
    expect(groups.map((g) => g.items.map((i) => i.id))).toEqual([[1, 3], [2]])
  })
})
