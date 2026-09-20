import { describe, expect, it } from 'vitest'
import { shiftDateInput, toDateInputValue, utcDayEnd, utcDayStart } from './useDates'

describe('date input helpers', () => {
  it('formats local calendar dates with zero padding', () => {
    expect(toDateInputValue(new Date(2025, 0, 6, 23, 59))).toBe('2025-01-06')
    expect(toDateInputValue(new Date(2025, 10, 30))).toBe('2025-11-30')
  })

  it('bounds a calendar day in UTC', () => {
    expect(utcDayStart('2025-01-06').toISOString()).toBe('2025-01-06T00:00:00.000Z')
    expect(utcDayEnd('2025-01-06').toISOString()).toBe('2025-01-06T23:59:59.000Z')
  })

  it('shifts across month and year ends', () => {
    expect(shiftDateInput('2025-01-31', 1)).toBe('2025-02-01')
    expect(shiftDateInput('2025-01-01', -1)).toBe('2024-12-31')
    expect(shiftDateInput('2024-02-28', 1)).toBe('2024-02-29')
  })
})
