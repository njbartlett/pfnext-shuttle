// Page state carried in the route query, both read-only inputs (?id=,
// ?email=) and state the page updates as the user navigates (?week=,
// ?edit=). Replaces the fragment-based useHashState of the per-page builds;
// legacy #key=value links are converted by the router (see router/index.ts).
import { computed } from 'vue'
import { useRoute, useRouter, type LocationQueryRaw } from 'vue-router'

export type QueryChanges = Record<string, string | null>

export function useQueryState() {
  const route = useRoute()
  const router = useRouter()

  // Changes made in the same tick are merged into a single history entry:
  // route.query only reflects a change once the navigation has completed,
  // so consecutive set() calls would otherwise overwrite each other
  let pending: QueryChanges | null = null

  function flush() {
    const changes = pending ?? {}
    pending = null
    const query: LocationQueryRaw = { ...route.query }
    for (const [key, value] of Object.entries(changes)) {
      if (value === null) {
        delete query[key]
      } else {
        query[key] = value
      }
    }
    void router.replace({ query })
  }

  function get(key: string): string | null {
    const value = route.query[key]
    const single = Array.isArray(value) ? value[0] : value
    return single ?? null
  }

  // Sets (or with null, removes) keys, leaving the others in place
  function update(changes: QueryChanges) {
    if (pending === null) {
      pending = {}
      queueMicrotask(flush)
    }
    Object.assign(pending, changes)
  }

  function set(key: string, value: string | null) {
    update({ [key]: value })
  }

  return { query: computed(() => route.query), get, set, update }
}
