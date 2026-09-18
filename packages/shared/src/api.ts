// Thin client for the pfnext JSON API. Errors arrive as {"code", "message"}
// bodies (backend src/apierror.rs) and are surfaced as ApiError instances.
//
// The client is configured once per app with configureApi(): the website
// talks to the unversioned "/api" mount with cookie credentials, the mobile
// app to the frozen "/api/v1" contract with a bearer token.

export interface ApiConfig {
  // Base URL including any version prefix, e.g. "/api" or "https://host/api/v1"
  baseUrl: string
  // Passed through to fetch(); the website needs 'include' for its session cookie
  credentials?: RequestCredentials
}

// The credits opt-in handshake: booking without credits_used when the session
// costs credits returns 402 with this code (backend src/bookings.rs).
export const CREDITS_OPT_IN_REQUIRED = 'credits_opt_in_required'

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

let config: ApiConfig = { baseUrl: '/api' }
let bearerToken: string | null = null
let onUnauthorized: (() => void) | null = null

export function configureApi(newConfig: ApiConfig) {
  config = { ...newConfig }
}

export function apiBaseUrl(): string {
  return config.baseUrl
}

export function setBearerToken(token: string | null) {
  bearerToken = token
}

export function setOnUnauthorized(handler: () => void) {
  onUnauthorized = handler
}

export interface RequestOptions {
  method?: string
  body?: unknown
  query?: Record<string, string | number | boolean | undefined>
  // Skip the global 401 handler, e.g. for the login call itself
  skipUnauthorizedHandler?: boolean
}

export async function apiRequest<T>(path: string, options: RequestOptions = {}): Promise<T> {
  const url = new URL(config.baseUrl + path, window.location.origin)
  for (const [key, value] of Object.entries(options.query ?? {})) {
    if (value !== undefined) {
      url.searchParams.set(key, String(value))
    }
  }

  const headers: Record<string, string> = {}
  if (bearerToken) {
    headers['Authorization'] = `Bearer ${bearerToken}`
  }
  if (options.body !== undefined) {
    headers['Content-Type'] = 'application/json'
  }

  const response = await fetch(url.toString(), {
    method: options.method ?? 'GET',
    headers,
    credentials: config.credentials,
    body: options.body !== undefined ? JSON.stringify(options.body) : undefined
  })

  if (!response.ok) {
    if (response.status === 401 && !options.skipUnauthorizedHandler) {
      onUnauthorized?.()
    }
    throw await toApiError(response)
  }

  if (response.status === 204) {
    return undefined as T
  }
  const text = await response.text()
  return (text ? JSON.parse(text) : undefined) as T
}

async function toApiError(response: Response): Promise<ApiError> {
  const text = await response.text().catch(() => '')
  try {
    const parsed = JSON.parse(text)
    if (parsed && typeof parsed.message === 'string') {
      return new ApiError(response.status, parsed.code ?? 'error', parsed.message)
    }
  } catch {
    // Not a JSON error body, fall through to the raw text
  }
  return new ApiError(response.status, 'error', text || `Request failed (${response.status})`)
}
