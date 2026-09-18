// Persists the login session on the device. Backed by the iOS Keychain and
// Android Keystore via @aparajita/capacitor-secure-storage; in a plain browser
// (dev server) the plugin falls back to encrypted localStorage.
import { SecureStorage } from '@aparajita/capacitor-secure-storage'
import type { LoggedInUser } from '@pfnext/shared'

const KEY = 'login'

export interface StoredLogin {
  token: string
  expiry: string
  user: LoggedInUser
}

export async function saveLogin(login: StoredLogin): Promise<void> {
  try {
    await SecureStorage.set(KEY, JSON.stringify(login))
  } catch (e) {
    console.error('Failed to persist login', e)
  }
}

export async function loadLogin(): Promise<StoredLogin | null> {
  try {
    const raw = await SecureStorage.get(KEY)
    if (typeof raw !== 'string' || !raw) {
      return null
    }
    const login = JSON.parse(raw) as StoredLogin
    if (!login.token || new Date(login.expiry) <= new Date()) {
      return null
    }
    return login
  } catch {
    return null
  }
}

export async function clearLogin(): Promise<void> {
  try {
    await SecureStorage.remove(KEY)
  } catch {
    // Nothing sensible to do; the token expires server-side after 7 days anyway
  }
}
