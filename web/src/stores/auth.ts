// Logged-in user state for the website. The session itself is a cookie set
// by POST /api/login; the user's details are mirrored in localStorage so a
// reload starts logged in without a round trip.
import { computed, ref } from 'vue'
import { login as loginRequest, logout as logoutRequest, setOnUnauthorized, type LoggedInUser } from '@pfnext/shared'

const LOGIN_STORAGE_KEY = 'anotherlevellogin'

const currentUser = ref<LoggedInUser | null>(readStoredUser())

export const user = computed(() => currentUser.value)
export const isLoggedIn = computed(() => currentUser.value !== null)
export const isAdmin = computed(() => hasRole('admin'))
export const isTrainer = computed(() => hasRole('trainer'))
// May write blog posts
export const isEditor = computed(() => isAdmin.value || hasRole('editor'))

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

export async function login(email: string, password: string): Promise<LoggedInUser> {
  setUser(null)
  const loggedIn = await loginRequest(email, password)
  setUser(loggedIn)
  return loggedIn
}

// Clears the login state; where the user ends up is the router's business
// (see AppNavbar and the isLoggedIn watch in App.vue)
export async function logout(): Promise<void> {
  setUser(null)
  try {
    await logoutRequest()
  } catch (e) {
    // Best effort: the local state is cleared regardless
    console.warn('Logout request failed', e)
  }
}
