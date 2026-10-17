// Read tracking on the blog: the post page records a logged-in member's
// first read, and the list marks published posts they have not read as new.
// Anonymous readers see the same posts with nothing recorded. API calls are
// mocked.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { VueWrapper } from '@vue/test-utils'
import { getPostByTitle, listPosts, markPostRead, type Post } from '@pfnext/shared'
import BlogIndexView from './BlogIndexView.vue'
import BlogPostView from './BlogPostView.vue'
import { apiError } from '@/stores/apiError'
import { setUser } from '@/stores/auth'
import { MEMBER } from '@/testing/fixtures'
import { mountAt } from '@/testing/mount'

vi.mock('@pfnext/shared', () => import('@/testing/sharedMock').then((m) => m.mockedShared()))

function makePost(overrides: Partial<Post> = {}): Post {
  return {
    id: 5,
    title: 'Park Run',
    author_id: 1,
    author_name: 'admin',
    author_email: 'admin@example.com',
    created_at: '2025-06-01T09:00:00Z',
    last_editor_id: 1,
    last_editor_name: 'admin',
    last_editor_email: 'admin@example.com',
    last_edited_at: '2025-06-01T09:00:00Z',
    published_at: '2025-06-01T09:00:00Z',
    read_at: null,
    content: '<p>We ran.</p>',
    ...overrides
  }
}

let wrapper: VueWrapper | undefined

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(markPostRead).mockResolvedValue(undefined)
})

afterEach(() => {
  wrapper?.unmount()
  wrapper = undefined
  setUser(null)
  expect(apiError.value).toBeNull()
})

describe('the post page', () => {
  async function openPost(post: Post) {
    vi.mocked(getPostByTitle).mockResolvedValue(post)
    ;({ wrapper } = await mountAt(BlogPostView, `/blog/posts/${encodeURIComponent(post.title)}.html`))
  }

  it('records a logged-in member reading a post for the first time', async () => {
    setUser(MEMBER)
    await openPost(makePost())
    expect(wrapper!.text()).toContain('We ran.')
    expect(markPostRead).toHaveBeenCalledWith(5)
  })

  it('does not record a post the member has already read', async () => {
    setUser(MEMBER)
    await openPost(makePost({ read_at: '2025-06-02T10:00:00Z' }))
    expect(markPostRead).not.toHaveBeenCalled()
  })

  it('shows the post to anonymous readers without recording anything', async () => {
    await openPost(makePost())
    expect(wrapper!.text()).toContain('We ran.')
    expect(markPostRead).not.toHaveBeenCalled()
  })

  it('still shows the post if recording the read fails', async () => {
    setUser(MEMBER)
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.mocked(markPostRead).mockRejectedValue(new Error('offline'))
    await openPost(makePost())
    expect(wrapper!.text()).toContain('We ran.')
    expect(warn).toHaveBeenCalled()
    warn.mockRestore()
  })
})

describe('the list of posts', () => {
  const POSTS = [
    makePost({ id: 1, title: 'Unread' }),
    makePost({ id: 2, title: 'Read', read_at: '2025-06-02T10:00:00Z' })
  ]

  function newBadgeTitles(): string[] {
    return wrapper!.findAll('li')
      .filter((item) => item.find('.badge').exists())
      .map((item) => item.find('a').text())
  }

  it('marks unread posts as new for a logged-in member', async () => {
    setUser(MEMBER)
    vi.mocked(listPosts).mockResolvedValue(POSTS)
    ;({ wrapper } = await mountAt(BlogIndexView, '/blog/index.html'))
    expect(newBadgeTitles()).toEqual(['Unread'])
  })

  it('marks nothing as new for anonymous readers', async () => {
    vi.mocked(listPosts).mockResolvedValue(POSTS.map((post) => ({ ...post, read_at: null })))
    ;({ wrapper } = await mountAt(BlogIndexView, '/blog/index.html'))
    expect(wrapper.findAll('li')).toHaveLength(2)
    expect(newBadgeTitles()).toEqual([])
  })
})
