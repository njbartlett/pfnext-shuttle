// Logged-in user state for the website. The session itself is a cookie set
// by POST /api/login; the user's details are mirrored in localStorage under
// the same key as static/js/library.js so that module pages and legacy pages
// agree on who is logged in during the migration.
import { computed, ref } from 'vue'
import { apiRequest, setOnUnauthorized, type LoggedInUser } from '@pfnext/shared'

const LOGIN_STORAGE_KEY = 'anotherlevellogin'

const currentUser = ref<LoggedInUser | null>(readStoredUser())

export const user = computed(() => currentUser.value)
export const isLoggedIn = computed(() => currentUser.value !== null)
export const isAdmin = computed(() => hasRole('admin'))
export const isTrainer = computed(() => hasRole('trainer'))

function hasRole(role: string): boolean {
  return currentUser.value?.roles?.includes(role) ?? false
}

function readStoredUser(): LoggedInUser | null {
  try {
    const raw = localStorage.getItem(LOGIN_STORAGE_KEY)
    return raw ? (JSON.parse(raw) as LoggedInUser) : null
  } catch {
    return null
  }
}

export function setUser(newUser: LoggedInUser | null) {
  currentUser.value = newUser
  try {
    if (newUser) {
      localStorage.setItem(LOGIN_STORAGE_KEY, JSON.stringify(newUser))
    } else {
      localStorage.removeItem(LOGIN_STORAGE_KEY)
    }
  } catch {
    // Storage unavailable (private mode etc.); the in-memory state still applies
  }
}

// The server rejected the session cookie (expired or revoked)
setOnUnauthorized(() => setUser(null))

// Returns the URL-encoded current location, for login.html?return=...
export function loginReturnUrl(): string {
  return encodeURIComponent(window.location.pathname + window.location.hash)
}

export async function logout(): Promise<void> {
  setUser(null)
  try {
    await apiRequest<void>('/logout', { method: 'POST' })
  } catch (e) {
    // Best effort: the local state is cleared regardless
    console.warn('Logout request failed', e)
  }
  window.location.href = '/'
}
