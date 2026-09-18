// A clock ref ticking once a second, for countdowns and "is this in the
// past yet" checks that must update without user interaction
import { onBeforeUnmount, ref, type Ref } from 'vue'

export function useNow(intervalMillis = 1000): Ref<Date> {
  const now = ref(new Date())
  const interval = setInterval(() => {
    now.value = new Date()
  }, intervalMillis)
  onBeforeUnmount(() => clearInterval(interval))
  return now
}

export function isPastAt(datetime: string | Date, now: Date): boolean {
  return new Date(datetime) < now
}

// True when datetime is within the next `minutes` from now
export function isSoon(datetime: string | Date, minutes: number, now: Date): boolean {
  const diffMinutes = (new Date(datetime).getTime() - now.getTime()) / 60000
  return diffMinutes >= 0 && diffMinutes <= minutes
}
