// Date/time display helpers, matching the en-GB conventions of the website.

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

export function isPast(datetime: string | Date): boolean {
  return new Date(datetime) < new Date()
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
