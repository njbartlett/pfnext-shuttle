import { afterEach, describe, expect, it } from 'vitest'
import { defineComponent, h, nextTick } from 'vue'
import { mount } from '@vue/test-utils'
import { useMediaQuery } from './useMediaQuery'

// jsdom has no real matchMedia (setup.ts stubs one that never matches), so
// this test installs its own and remembers the change listeners
const originalMatchMedia = window.matchMedia
let listeners: ((event: MediaQueryListEvent) => void)[] = []

function stubMatchMedia(matches: boolean) {
  listeners = []
  window.matchMedia = (query: string) =>
    ({
      matches,
      media: query,
      addEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => listeners.push(listener),
      removeEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => {
        listeners = listeners.filter((l) => l !== listener)
      }
    }) as unknown as MediaQueryList
}

afterEach(() => {
  window.matchMedia = originalMatchMedia
})

describe('useMediaQuery', () => {
  it('starts from the current match, follows changes and stops listening on unmount', async () => {
    stubMatchMedia(true)
    const wrapper = mount(
      defineComponent({
        setup() {
          const narrow = useMediaQuery('(max-width: 767.98px)')
          return () => h('div', narrow.value ? 'narrow' : 'wide')
        }
      })
    )
    expect(wrapper.text()).toBe('narrow')

    listeners.forEach((listener) => listener({ matches: false } as MediaQueryListEvent))
    await nextTick()
    expect(wrapper.text()).toBe('wide')

    wrapper.unmount()
    expect(listeners).toHaveLength(0)
  })
})
