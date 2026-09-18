// Paging over an unbounded sequence of periods (weeks, months) relative to
// "now" (offset 0). The bar shows a block of `pageCount` consecutive offsets
// starting at block + firstIndex; back/forward slide the block.
import { computed, ref } from 'vue'

export function usePagedWindow(pageCount = 4, firstIndex = -1) {
  const block = ref(0)
  const offset = ref(0)

  const indices = computed(() => Array.from({ length: pageCount }, (_, i) => block.value + firstIndex + i))
  const home = computed(() => block.value === 0 && offset.value === 0)

  function back() {
    block.value -= pageCount
  }

  function forward() {
    block.value += pageCount
  }

  function reset() {
    block.value = 0
    offset.value = 0
  }

  // Slides the block so that `target` is visible, keeping blocks aligned to
  // multiples of pageCount
  function jumpTo(target: number) {
    offset.value = target
    const sign = target >= 0 ? 1 : -1
    block.value = sign * Math.floor(Math.abs(target) / pageCount) * pageCount
  }

  return { block, offset, indices, home, back, forward, reset, jumpTo }
}
