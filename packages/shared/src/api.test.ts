import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ApiError, apiRequest, configureApi, setBearerToken, setOnUnauthorized } from './api'

const fetchMock = vi.fn<typeof fetch>()

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } })
}

function requestedUrl(): URL {
  return new URL(String(fetchMock.mock.calls[0][0]))
}

function requestedInit(): RequestInit {
  return fetchMock.mock.calls[0][1] ?? {}
}

beforeEach(() => {
  vi.stubGlobal('fetch', fetchMock)
  configureApi({ baseUrl: '/api', credentials: 'include' })
  setBearerToken(null)
})

afterEach(() => {
  fetchMock.mockReset()
  vi.unstubAllGlobals()
})

describe('request building', () => {
  it('resolves the path against the configured base and repeats array query keys', async () => {
    fetchMock.mockResolvedValue(jsonResponse([]))
    await apiRequest('/sessions', { query: { ids: [1, 2], from: 'x', skip: undefined, none: null, flag: false } })

    const url = requestedUrl()
    expect(url.pathname).toBe('/api/sessions')
    expect(url.searchParams.getAll('ids')).toEqual(['1', '2'])
    expect(url.searchParams.get('from')).toBe('x')
    expect(url.searchParams.get('flag')).toBe('false')
    expect(url.searchParams.has('skip')).toBe(false)
    expect(url.searchParams.has('none')).toBe(false)
    expect(requestedInit().credentials).toBe('include')
    expect(requestedInit().method).toBe('GET')
  })

  it('sends JSON bodies with a content type', async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }))
    await apiRequest('/bookings', { method: 'POST', body: { person_id: 1 } })

    expect(requestedInit().method).toBe('POST')
    expect(requestedInit().body).toBe('{"person_id":1}')
    expect((requestedInit().headers as Record<string, string>)['Content-Type']).toBe('application/json')
  })

  it('adds the bearer token when one is set', async () => {
    fetchMock.mockResolvedValue(jsonResponse({}))
    setBearerToken('abc')
    await apiRequest('/users/1')
    expect((requestedInit().headers as Record<string, string>)['Authorization']).toBe('Bearer abc')
  })

  it('uses an absolute base URL as given', async () => {
    configureApi({ baseUrl: 'https://example.test/api/v1' })
    fetchMock.mockResolvedValue(jsonResponse({}))
    await apiRequest('/users/1')
    expect(String(fetchMock.mock.calls[0][0])).toBe('https://example.test/api/v1/users/1')
  })
})

describe('response handling', () => {
  it('parses JSON responses', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ id: 7 }))
    await expect(apiRequest<{ id: number }>('/users/7')).resolves.toEqual({ id: 7 })
  })

  it('returns plain text responses as text', async () => {
    fetchMock.mockResolvedValue(new Response('Reset email sent', { status: 200, headers: { 'Content-Type': 'text/plain' } }))
    await expect(apiRequest<string>('/reset')).resolves.toBe('Reset email sent')
  })

  it('returns undefined for 204 and empty bodies', async () => {
    fetchMock.mockResolvedValueOnce(new Response(null, { status: 204 }))
    await expect(apiRequest('/a')).resolves.toBeUndefined()
    fetchMock.mockResolvedValueOnce(new Response('', { status: 200 }))
    await expect(apiRequest('/b')).resolves.toBeUndefined()
  })
})

describe('errors', () => {
  it('surfaces the server error code and message', async () => {
    fetchMock.mockResolvedValue(jsonResponse({ code: 'credits_opt_in_required', message: 'Needs credits' }, 402))
    const error = await apiRequest('/bookings').catch((e) => e)
    expect(error).toBeInstanceOf(ApiError)
    expect(error).toMatchObject({ status: 402, code: 'credits_opt_in_required', message: 'Needs credits' })
  })

  it('falls back to the raw body, or a generic message, for non-JSON errors', async () => {
    fetchMock.mockResolvedValueOnce(new Response('Bad gateway', { status: 502 }))
    await expect(apiRequest('/a')).rejects.toMatchObject({ status: 502, code: 'error', message: 'Bad gateway' })
    fetchMock.mockResolvedValueOnce(new Response('', { status: 500 }))
    await expect(apiRequest('/b')).rejects.toMatchObject({ status: 500, message: 'Request failed (500)' })
  })

  it('calls the unauthorised handler on 401 unless the caller opts out', async () => {
    const handler = vi.fn()
    setOnUnauthorized(handler)
    fetchMock.mockResolvedValue(jsonResponse({ code: 'unauthorized', message: 'expired' }, 401))

    await expect(apiRequest('/users/1')).rejects.toBeInstanceOf(ApiError)
    expect(handler).toHaveBeenCalledTimes(1)

    await expect(apiRequest('/login', { skipUnauthorizedHandler: true })).rejects.toBeInstanceOf(ApiError)
    expect(handler).toHaveBeenCalledTimes(1)
  })
})
