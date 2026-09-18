import { computed, ref } from 'vue'
import { apiRequest, setBearerToken, setOnUnauthorized } from '@pfnext/shared'
import type { LoggedInUser } from '@pfnext/shared'
import { clearLogin, loadLogin, saveLogin } from './tokenStorage'

const currentUser = ref<LoggedInUser | null>(null)
const restored = ref(false)

export const user = computed(() => currentUser.value)
export const isAuthenticated = computed(() => currentUser.value !== null)

// The server rejected our token (expired or revoked): drop the local session.
// The router guard redirects to the login page on the next navigation.
setOnUnauthorized(() => {
  void clearLocalSession()
})

// Restore a persisted session at app start. Resolves once, before routing.
export async function restoreSession(): Promise<void> {
  if (restored.value) {
    return
  }
  const stored = await loadLogin()
  if (stored) {
    setBearerToken(stored.token)
    currentUser.value = stored.user
  }
  restored.value = true
}

export async function login(email: string, password: string): Promise<void> {
  const response = await apiRequest<LoggedInUser>('/login', {
    method: 'POST',
    body: { email, password, client: 'mobile' },
    skipUnauthorizedHandler: true
  })
  if (!response.token || !response.expiry) {
    throw new Error('Server did not return a session token')
  }
  setBearerToken(response.token)
  currentUser.value = response
  await saveLogin({ token: response.token, expiry: response.expiry, user: response })
}

export async function logout(): Promise<void> {
  try {
    await apiRequest<void>('/logout', { method: 'POST' })
  } catch (e) {
    // Best effort: the local session is cleared regardless
    console.warn('Logout request failed', e)
  }
  await clearLocalSession()
}

async function clearLocalSession(): Promise<void> {
  setBearerToken(null)
  currentUser.value = null
  await clearLogin()
}
