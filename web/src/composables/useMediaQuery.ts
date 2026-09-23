// Whether a CSS media query currently matches, kept up to date as the window
// is resized or the phone rotated. Use it where the layout, not just the
// styling, has to change with the screen size.
import { onBeforeUnmount, readonly, ref, type Ref } from 'vue'

export function useMediaQuery(query: string): Readonly<Ref<boolean>> {
  const list = window.matchMedia(query)
  const matches = ref(list.matches)
  const onChange = (event: MediaQueryListEvent) => {
    matches.value = event.matches
  }
  list.addEventListener('change', onChange)
  onBeforeUnmount(() => list.removeEventListener('change', onChange))
  return readonly(matches)
}
