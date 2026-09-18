// Tracks whether an editable copy of a record differs from the last saved
// version, for enabling a Save button
import { computed, ref, type Ref } from 'vue'

export function useDirtyTracking<T extends object>(draft: Ref<T> | T) {
  const saved = ref<string | null>(null)

  function snapshot(value: T): string {
    return JSON.stringify(value)
  }

  function current(): T {
    return 'value' in draft && !(draft instanceof Array) ? (draft as Ref<T>).value : (draft as T)
  }

  // Call after loading or saving to mark the current draft as clean
  function markClean() {
    saved.value = snapshot(current())
  }

  const dirty = computed(() => saved.value !== null && snapshot(current()) !== saved.value)

  return { dirty, markClean }
}
