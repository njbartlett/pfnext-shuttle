// The `?return=` convention: focused pages (login, editors, attendance) are
// opened with the path to go back to. Replaces the page_return_path global.

export function useReturnPath(fallback: string | null = null) {
  const raw = new URLSearchParams(window.location.search).get('return')
  const returnPath = raw ? decodeURIComponent(raw) : fallback

  function goBack() {
    if (returnPath) {
      window.location.href = returnPath
    } else {
      window.history.back()
    }
  }

  return { returnPath, goBack }
}
