import { describe, expect, it } from 'vitest'
import { usePagedWindow } from './usePagedWindow'

describe('usePagedWindow', () => {
  it('starts on the block around now', () => {
    const pager = usePagedWindow()
    expect(pager.indices.value).toEqual([-1, 0, 1, 2])
    expect(pager.offset.value).toBe(0)
    expect(pager.home.value).toBe(true)
  })

  it('slides whole blocks back and forward, and resets', () => {
    const pager = usePagedWindow()
    pager.forward()
    expect(pager.indices.value).toEqual([3, 4, 5, 6])
    pager.back()
    pager.back()
    expect(pager.indices.value).toEqual([-5, -4, -3, -2])
    expect(pager.home.value).toBe(false)
    pager.reset()
    expect(pager.indices.value).toEqual([-1, 0, 1, 2])
    expect(pager.home.value).toBe(true)
  })

  it('is not home while a different offset is selected', () => {
    const pager = usePagedWindow()
    pager.offset.value = 1
    expect(pager.home.value).toBe(false)
  })

  describe('jumpTo', () => {
    // Every target must land inside the visible block, and blocks stay
    // aligned so that stepping back/forward from there reaches "now"
    it.each([-9, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 6, 7, 9, 10])('shows offset %i', (target) => {
      const pager = usePagedWindow()
      pager.jumpTo(target)
      expect(pager.offset.value).toBe(target)
      expect(pager.indices.value).toContain(target)
      expect(Math.abs(pager.block.value % 4)).toBe(0)
    })

    it('keeps the current block for offsets already visible', () => {
      const pager = usePagedWindow()
      pager.jumpTo(2)
      expect(pager.indices.value).toEqual([-1, 0, 1, 2])
      pager.jumpTo(-1)
      expect(pager.indices.value).toEqual([-1, 0, 1, 2])
    })

    it('honours other page sizes and first indices', () => {
      const pager = usePagedWindow(3, 0)
      pager.jumpTo(5)
      expect(pager.indices.value).toEqual([3, 4, 5])
      pager.jumpTo(-1)
      expect(pager.indices.value).toEqual([-3, -2, -1])
    })
  })
})
