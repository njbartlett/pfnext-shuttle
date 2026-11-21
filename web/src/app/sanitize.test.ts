import { describe, expect, it } from 'vitest'
import { sanitizeHtml } from './sanitize'

describe('sanitizeHtml', () => {
  it('keeps ordinary formatting', () => {
    const html = '<p>Hello <strong>world</strong></p><ul><li>one</li></ul><a href="https://example.org">link</a><img src="https://example.org/a.png" alt="a">'
    expect(sanitizeHtml(html)).toBe(html)
  })

  it('removes scripts and event handlers', () => {
    expect(sanitizeHtml('<p>hi</p><script>alert(1)</script>')).toBe('<p>hi</p>')
    expect(sanitizeHtml('<img src="x" onerror="alert(1)">')).toBe('<img src="x">')
    expect(sanitizeHtml('<a href="javascript:alert(1)">x</a>')).toBe('<a>x</a>')
  })

  it('treats missing content as empty', () => {
    expect(sanitizeHtml(null)).toBe('')
    expect(sanitizeHtml(undefined)).toBe('')
  })
})
