// Runs before every web test file (vitest.config.ts setupFiles). Fills the
// gaps in jsdom that the app relies on and makes sure no test reaches the
// network: components get their data from the mocked @pfnext/shared module
// (see sharedMock.ts), never from Rocket.
import { afterEach, vi } from 'vitest'

// useTheme reads the system colour scheme through matchMedia, which jsdom lacks
if (typeof window.matchMedia !== 'function') {
  window.matchMedia = (query: string): MediaQueryList =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
      addListener: () => undefined,
      removeListener: () => undefined,
      dispatchEvent: () => false
    }) as MediaQueryList
}

// The router's scrollBehavior scrolls after each navigation; jsdom logs
// "not implemented" for the real one
window.scrollTo = () => undefined

vi.stubGlobal('fetch', (input: RequestInfo | URL) =>
  Promise.reject(new Error(`Unexpected network request to ${String(input)} in a component test`))
)

afterEach(() => {
  // Undoes vi.setSystemTime() as well as any fake timers
  vi.useRealTimers()
  localStorage.clear()
})
