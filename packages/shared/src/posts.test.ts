import { describe, expect, it } from 'vitest'
import { postLocation, postTitle } from './posts'

describe('post locations', () => {
  it('round-trips titles with spaces and punctuation', () => {
    const title = 'Park Run & Picnic: 100% fun'
    expect(postLocation(title)).toBe('Park Run & Picnic: 100% fun.html')
    expect(postTitle(postLocation(title))).toBe(title)
  })

  it('rejects locations that are not post pages', () => {
    expect(postTitle('new')).toBeNull()
    expect(postTitle('')).toBeNull()
  })
})
