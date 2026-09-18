// Page-level error banner state, shown by ApiErrorAlert in the page shell.
// Replaces the global `http_err` of static/js/library.js.
import { ref } from 'vue'
import { ApiError } from '@pfnext/shared'

export const apiError = ref<ApiError | null>(null)

export function reportApiError(error: unknown) {
  if (error instanceof ApiError) {
    apiError.value = error
  } else {
    const message = error instanceof Error ? error.message : String(error)
    apiError.value = new ApiError(0, 'error', message)
  }
}

export function clearApiError() {
  apiError.value = null
}

// Runs an API call, clearing the banner first and showing any failure in
// it. Resolves to undefined on failure so callers can `if (result)`.
export async function tryApi<T>(call: () => Promise<T>): Promise<T | undefined> {
  clearApiError()
  try {
    return await call()
  } catch (error) {
    reportApiError(error)
    return undefined
  }
}
