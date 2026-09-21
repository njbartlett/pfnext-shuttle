// The mocked @pfnext/shared module for component tests. Each test file that
// mounts a view opts in with:
//
//   vi.mock('@pfnext/shared', () => import('@/testing/sharedMock').then((m) => m.mockedShared()))
//
// Every function in the shared package that talks to the API is async, and
// none of the pure helpers (formatting, isFull, validation, the ApiError
// class) are, so that is the line: async exports become vi.fn() stubs that
// reject until a test gives them a value with vi.mocked(fn).mockResolvedValue;
// everything else is the real implementation.
import { vi } from 'vitest'

type Shared = typeof import('@pfnext/shared')

export async function mockedShared(): Promise<Shared> {
  const actual = await vi.importActual<Shared>('@pfnext/shared')
  const mocked: Record<string, unknown> = { ...actual }
  for (const [name, value] of Object.entries(actual)) {
    if (isAsyncFunction(value)) {
      mocked[name] = vi.fn(() => Promise.reject(new Error(`API call ${name}() was not mocked by the test`)))
    }
  }
  return mocked as unknown as Shared
}

function isAsyncFunction(value: unknown): boolean {
  return typeof value === 'function' && value.constructor.name === 'AsyncFunction'
}
