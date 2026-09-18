// Helpers for the YYYY-MM-DD strings that <input type="date"> works with

// Local calendar date of `date` as YYYY-MM-DD
export function toDateInputValue(date: Date): string {
  return (
    String(date.getFullYear()).padStart(4, '0') +
    '-' +
    String(date.getMonth() + 1).padStart(2, '0') +
    '-' +
    String(date.getDate()).padStart(2, '0')
  )
}

// Start of the given calendar day in UTC, matching how the legacy pages
// built their from/to query bounds
export function utcDayStart(dateInput: string): Date {
  return new Date(dateInput + 'T00:00:00Z')
}

export function utcDayEnd(dateInput: string): Date {
  return new Date(dateInput + 'T23:59:59Z')
}

export function shiftDateInput(dateInput: string, days: number): string {
  const date = new Date(dateInput)
  date.setDate(date.getDate() + days)
  return toDateInputValue(date)
}
