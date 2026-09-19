// The `?return=` convention: focused pages (login, editors, attendance) are
// opened with the route to go back to afterwards.
import { useRoute, useRouter } from 'vue-router'

export function useReturnPath(fallback: string | null = null) {
  const route = useRoute()
  const router = useRouter()

  const raw = route.query.return
  const returnPath = typeof raw === 'string' && raw !== '' ? raw : fallback

  function goBack() {
    if (returnPath) {
      void router.push(returnPath)
    } else {
      router.back()
    }
  }

  return { returnPath, goBack }
}
