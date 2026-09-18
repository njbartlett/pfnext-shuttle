// Page state carried in the URL. Query parameters are read-only inputs
// (?id=, ?email=); the fragment holds state the page updates as the user
// navigates (#week=, #edit=). In the single-page-app step both become
// route queries; views only use this module, so they will not change.
import { readonly, ref } from 'vue'

export function urlQuery(): URLSearchParams {
  return new URLSearchParams(window.location.search)
}

// Absolute URL of the password reset page, sent to the server for emails
export function passwordResetUrl(): string {
  return new URL('/passwordreset.html', window.location.href).href
}

function parseHash(): URLSearchParams {
  const hash = window.location.hash
  return new URLSearchParams(hash ? hash.substring(1) : '')
}

const hashParams = ref(parseHash())
window.addEventListener('hashchange', () => {
  hashParams.value = parseHash()
})

export function useHashState() {
  function get(key: string): string | null {
    return hashParams.value.get(key)
  }

  // Sets (or with null, removes) one key, leaving the others in place
  function set(key: string, value: string | null) {
    const params = parseHash()
    if (value === null) {
      params.delete(key)
    } else {
      params.set(key, value)
    }
    window.location.hash = params.toString()
    hashParams.value = params
  }

  return { params: readonly(hashParams), get, set }
}
