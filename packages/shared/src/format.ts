// Date/time and text display helpers, matching the en-GB conventions of the
// website.
//
// Two families of date helpers exist on purpose. displayDate/displayTime
// format in the browser's own time zone in short form (the mobile app's
// convention). The "venue" and "full" variants pin the club's time zone,
// which is what the website shows: session times are local to the venue.

export const TIMEZONE = 'Europe/London'
export const START_OF_WEEK = 1 // Monday

export function displayDate(datetime: string | Date): string {
  return new Date(datetime).toLocaleDateString('en-GB', {
    weekday: 'short',
    day: 'numeric',
    month: 'short'
  })
}

export function displayLongDate(datetime: string | Date): string {
  return new Date(datetime).toLocaleDateString('en-GB', {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
    year: 'numeric'
  })
}

export function displayTime(datetime: string | Date): string {
  return new Date(datetime).toLocaleTimeString('en-GB', {
    hour: '2-digit',
    minute: '2-digit'
  })
}

// "Mon, 6 January 2025" in the venue's time zone
export function displayFullDate(datetime: string | Date): string {
  return new Date(datetime).toLocaleDateString('en-GB', {
    timeZone: TIMEZONE,
    weekday: 'short',
    day: 'numeric',
    month: 'long',
    year: 'numeric'
  })
}

// "Mon 6 Jan" in the venue's time zone: the abbreviated companion to
// displayFullDate for narrow screens, where the year is left out
export function displayVenueDate(datetime: string | Date): string {
  return new Date(datetime)
    .toLocaleDateString('en-GB', {
      timeZone: TIMEZONE,
      weekday: 'short',
      day: 'numeric',
      month: 'short'
    })
    .replace(',', '')
}

// Days since the epoch of the venue-zone calendar date, so that two moments
// can be compared as venue days regardless of the browser's zone or DST
function venueDayNumber(datetime: string | Date): number {
  const [year, month, day] = new Date(datetime)
    .toLocaleDateString('en-CA', { timeZone: TIMEZONE })
    .split('-')
    .map(Number)
  return Date.UTC(year, month - 1, day) / 86400000
}

// "Yesterday", "Today" or "Tomorrow" when datetime falls on one of those
// venue days relative to `now`; null for any other day, so callers can fall
// back to a date
export function displayRelativeDay(datetime: string | Date, now: Date = new Date()): string | null {
  switch (venueDayNumber(datetime) - venueDayNumber(now)) {
    case -1:
      return 'Yesterday'
    case 0:
      return 'Today'
    case 1:
      return 'Tomorrow'
    default:
      return null
  }
}

// "18:30" in the venue's time zone
export function displayVenueTime(datetime: string | Date): string {
  return new Date(datetime).toLocaleTimeString('en-GB', {
    timeZone: TIMEZONE,
    timeStyle: 'short',
    hour12: false
  })
}

// "6 January 2025" in the venue's time zone
export function displayDayMonthYear(datetime: string | Date): string {
  return new Date(datetime).toLocaleDateString('en-GB', {
    timeZone: TIMEZONE,
    day: 'numeric',
    month: 'long',
    year: 'numeric'
  })
}

// "6 Jan 2025 18:30:00" in the venue's time zone
export function displayDateTime(datetime: string | Date | null): string | null {
  if (datetime === null) {
    return null
  }
  const value = new Date(datetime)
  return (
    value.toLocaleDateString('en-GB', {
      timeZone: TIMEZONE,
      day: 'numeric',
      month: 'short',
      year: 'numeric'
    }) +
    ' ' +
    value.toLocaleTimeString('en-GB', { timeZone: TIMEZONE })
  )
}

// "6 – 12 Jan 2025", "30 Dec 2024 – 5 Jan 2025"
export function displayDateRange(start: Date, end: Date): string {
  const startOptions: Intl.DateTimeFormatOptions = { day: 'numeric' }
  if (start.getMonth() !== end.getMonth()) {
    startOptions.month = 'short'
  }
  if (start.getFullYear() !== end.getFullYear()) {
    startOptions.year = 'numeric'
  }
  return (
    start.toLocaleDateString('en-GB', startOptions) +
    ' – ' +
    end.toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })
  )
}

export function displayNumber(value: number): string {
  return value.toLocaleString('en-GB')
}

export function displayPercent(value: number, places: number): string {
  return value.toLocaleString(undefined, {
    style: 'percent',
    minimumFractionDigits: places,
    maximumFractionDigits: places
  })
}

export function formatNameAndEmail(name: string, email: string): string {
  return `${name} <${email}>`
}

// "~ 2d 3h", "3h 12m", "12m 5s", "5s"
export function formatDuration(millis: number): string {
  let ms = Math.abs(millis)
  const days = Math.floor(ms / 86400000)
  const hours = Math.floor((ms % 86400000) / 3600000)
  const mins = Math.floor((ms % 3600000) / 60000)
  const secs = Math.floor((ms % 60000) / 1000)

  if (days > 0) {
    return `~ ${days}d ${hours}h`
  }
  if (hours > 0) {
    return `${hours}h ${mins}m`
  }
  if (mins > 0) {
    return `${mins}m ${secs}s`
  }
  return `${secs}s`
}

export function isPast(datetime: string | Date): boolean {
  return new Date(datetime) < new Date()
}

export function startOfDay(date: Date): Date {
  const start = new Date(date)
  start.setHours(0, 0, 0, 0)
  return start
}

export function startOfWeek(date: Date): Date {
  const start = startOfDay(date)
  start.setDate(start.getDate() - ((start.getDay() + 7 - START_OF_WEEK) % 7))
  return start
}

export function startOfMonth(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), 1, 0, 0, 0, 0)
}

// Last millisecond of the month
export function endOfMonth(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth() + 1, 0, 23, 59, 59, 999)
}

export function addDays(date: Date, days: number): Date {
  const result = new Date(date)
  result.setDate(result.getDate() + days)
  return result
}

export function addMonths(date: Date, months: number): Date {
  const result = new Date(date)
  result.setMonth(result.getMonth() + months)
  return result
}

export function isSameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
}

// Groups items carrying a datetime into calendar days, in ascending order
export function groupByDay<T>(items: T[], datetimeOf: (item: T) => string): { day: string; items: T[] }[] {
  const groups = new Map<string, T[]>()
  for (const item of items) {
    const day = new Date(datetimeOf(item)).toDateString()
    const group = groups.get(day)
    if (group) {
      group.push(item)
    } else {
      groups.set(day, [item])
    }
  }
  return [...groups.entries()].map(([day, items]) => ({ day, items }))
}

// Sorts in place by one field; nulls first ascending, last descending
export function sortByField<T>(items: T[], field: keyof T, ascending = true): T[] {
  return items.sort((a, b) => {
    const valueA = a[field]
    const valueB = b[field]
    const aIsNull = valueA == null
    const bIsNull = valueB == null
    if (aIsNull && bIsNull) return 0
    if (aIsNull) return ascending ? -1 : 1
    if (bIsNull) return ascending ? 1 : -1
    if (typeof valueA === 'string' && typeof valueB === 'string') {
      return ascending ? valueA.localeCompare(valueB) : valueB.localeCompare(valueA)
    }
    return ascending ? Number(valueA) - Number(valueB) : Number(valueB) - Number(valueA)
  })
}
