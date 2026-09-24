import { describe, expect, it, vi } from 'vitest'
import { defineComponent, h, ref } from 'vue'
import { mount } from '@vue/test-utils'
import { useSwipe } from './useSwipe'

// jsdom has no Touch constructor, so touches are plain objects on a plain
// event, which is all the composable reads
function touch(element: Element, type: 'touchstart' | 'touchend' | 'touchcancel', points: { x: number; y: number }[]) {
  const event = new Event(type)
  const touches = points.map((p) => ({ clientX: p.x, clientY: p.y }))
  Object.defineProperty(event, 'touches', { value: type === 'touchend' ? [] : touches })
  Object.defineProperty(event, 'changedTouches', { value: touches })
  element.dispatchEvent(event)
}

function mountSwipeable() {
  const onSwipeLeft = vi.fn()
  const onSwipeRight = vi.fn()
  const wrapper = mount(
    defineComponent({
      setup() {
        const frame = ref<HTMLElement | null>(null)
        useSwipe(frame, { onSwipeLeft, onSwipeRight })
        return () => h('div', { ref: frame })
      }
    })
  )
  return { wrapper, element: wrapper.element, onSwipeLeft, onSwipeRight }
}

describe('useSwipe', () => {
  it('reports a horizontal swipe in either direction', () => {
    const { element, onSwipeLeft, onSwipeRight } = mountSwipeable()

    touch(element, 'touchstart', [{ x: 200, y: 100 }])
    touch(element, 'touchend', [{ x: 120, y: 110 }])
    expect(onSwipeLeft).toHaveBeenCalledTimes(1)

    touch(element, 'touchstart', [{ x: 100, y: 100 }])
    touch(element, 'touchend', [{ x: 160, y: 90 }])
    expect(onSwipeRight).toHaveBeenCalledTimes(1)
  })

  it('ignores short, diagonal, two-finger and cancelled touches', () => {
    const { element, onSwipeLeft, onSwipeRight } = mountSwipeable()

    touch(element, 'touchstart', [{ x: 200, y: 100 }])
    touch(element, 'touchend', [{ x: 170, y: 100 }]) // too short
    touch(element, 'touchstart', [{ x: 200, y: 100 }])
    touch(element, 'touchend', [{ x: 120, y: 160 }]) // mostly a scroll
    touch(element, 'touchstart', [{ x: 200, y: 100 }, { x: 220, y: 100 }])
    touch(element, 'touchend', [{ x: 100, y: 100 }]) // pinch
    touch(element, 'touchstart', [{ x: 200, y: 100 }])
    touch(element, 'touchcancel', [])
    touch(element, 'touchend', [{ x: 100, y: 100 }]) // after a cancel

    expect(onSwipeLeft).not.toHaveBeenCalled()
    expect(onSwipeRight).not.toHaveBeenCalled()
  })

  it('halts any native scrolling once a swipe is recognised, and only then', () => {
    const scrollTo = vi.spyOn(window, 'scrollTo').mockImplementation(() => undefined)
    Object.defineProperty(window, 'scrollX', { value: 0, configurable: true })
    Object.defineProperty(window, 'scrollY', { value: 340, configurable: true })
    const { element } = mountSwipeable()

    touch(element, 'touchstart', [{ x: 200, y: 100 }])
    touch(element, 'touchend', [{ x: 170, y: 100 }]) // too short: an ordinary touch
    expect(scrollTo).not.toHaveBeenCalled()

    touch(element, 'touchstart', [{ x: 200, y: 100 }])
    touch(element, 'touchend', [{ x: 120, y: 110 }])
    expect(scrollTo).toHaveBeenCalledWith({ left: 0, top: 340, behavior: 'instant' })
    scrollTo.mockRestore()
  })

  it('stops listening once unmounted', () => {
    const { wrapper, element, onSwipeLeft } = mountSwipeable()
    wrapper.unmount()
    touch(element, 'touchstart', [{ x: 200, y: 100 }])
    touch(element, 'touchend', [{ x: 100, y: 100 }])
    expect(onSwipeLeft).not.toHaveBeenCalled()
  })
})
