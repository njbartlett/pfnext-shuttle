// Horizontal swipes on a touch screen, for paging. A swipe is one finger
// moving at least `threshold` pixels and clearly more sideways than up or
// down, so ordinary scrolling is left alone. Listeners are attached to the
// element once the template ref is filled and removed when it changes or
// the component unmounts.
import { onBeforeUnmount, watch, type Ref } from 'vue'

export interface SwipeHandlers {
  onSwipeLeft?: () => void
  onSwipeRight?: () => void
}

export function useSwipe(target: Ref<HTMLElement | null>, handlers: SwipeHandlers, threshold = 50) {
  let start: { x: number; y: number } | null = null

  function onTouchStart(event: TouchEvent) {
    const touch = event.touches.length === 1 ? event.touches[0] : null
    start = touch ? { x: touch.clientX, y: touch.clientY } : null
  }

  function onTouchEnd(event: TouchEvent) {
    const touch = event.changedTouches[0]
    if (!start || !touch) {
      start = null
      return
    }
    const dx = touch.clientX - start.x
    const dy = touch.clientY - start.y
    start = null
    if (Math.abs(dx) >= threshold && Math.abs(dx) > 2 * Math.abs(dy)) {
      haltNativeScrolling()
      ;(dx < 0 ? handlers.onSwipeLeft : handlers.onSwipeRight)?.()
    }
  }

  // A swipe with any vertical drift can set the page scrolling by a few
  // pixels, and iOS keeps that scroll "decelerating" for a second or more
  // after it has visibly stopped. A tap landing in that time is treated by
  // WebKit as interrupting the scroll: it drops the touch start, and since
  // iOS 18.4 then ignores the tap outright when it is far from where the
  // swipe began (WebKit bug 284346), so the button the user paged to has to
  // be tapped twice. Scrolling to where we already are ends the deceleration.
  function haltNativeScrolling() {
    window.scrollTo({ left: window.scrollX, top: window.scrollY, behavior: 'instant' })
  }

  function onTouchCancel() {
    start = null
  }

  function attach(element: HTMLElement) {
    element.addEventListener('touchstart', onTouchStart, { passive: true })
    element.addEventListener('touchend', onTouchEnd, { passive: true })
    element.addEventListener('touchcancel', onTouchCancel, { passive: true })
  }

  function detach(element: HTMLElement) {
    element.removeEventListener('touchstart', onTouchStart)
    element.removeEventListener('touchend', onTouchEnd)
    element.removeEventListener('touchcancel', onTouchCancel)
  }

  // Synchronous so the element listens from the moment the ref is filled
  watch(
    target,
    (element, previous) => {
      if (previous) detach(previous)
      if (element) attach(element)
    },
    { immediate: true, flush: 'sync' }
  )
  onBeforeUnmount(() => {
    if (target.value) detach(target.value)
  })
}
