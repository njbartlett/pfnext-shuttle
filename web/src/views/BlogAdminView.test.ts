// The editors' list of all posts: drafts, scheduled and published posts with
// read counts, and the publish/unpublish and delete actions. API calls are
// mocked.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, type VueWrapper } from '@vue/test-utils'
import { deletePost, listPosts, setPostPublication } from '@pfnext/shared'
import BlogAdminView from './BlogAdminView.vue'
import ConfirmModal from '@/components/ConfirmModal.vue'
import { apiError } from '@/stores/apiError'
import { setUser } from '@/stores/auth'
import { ADMIN, makePost, MEMBER } from '@/testing/fixtures'
import { mountAt } from '@/testing/mount'

vi.mock('@pfnext/shared', () => import('@/testing/sharedMock').then((m) => m.mockedShared()))

const POSTS = [
  makePost({ id: 1, title: 'Draft', published_at: null, read_count: 0 }),
  makePost({ id: 2, title: 'Scheduled', published_at: '2999-01-01T09:00:00Z', read_count: 0 }),
  makePost({ id: 3, title: 'Published', read_count: 7 })
]

let wrapper: VueWrapper | undefined

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(listPosts).mockResolvedValue(POSTS)
})

afterEach(() => {
  wrapper?.unmount()
  wrapper = undefined
  setUser(null)
  expect(apiError.value).toBeNull()
})

async function open() {
  ;({ wrapper } = await mountAt(BlogAdminView, '/blog/admin.html', { attachTo: document.body }))
}

function row(postId: number) {
  return wrapper!.get(`#post-${postId}`)
}

// Bootstrap modals open and close on a timer, so wait for their events,
// which bubble to the document
function modalEvent(name: 'shown.bs.modal' | 'hidden.bs.modal'): Promise<void> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`No ${name} event within 1s`)), 1000)
    document.addEventListener(name, () => {
      clearTimeout(timer)
      resolve()
    }, { once: true })
  })
}

describe('the blog admin page', () => {
  it('lists every post with its status and read count', async () => {
    setUser(ADMIN)
    await open()
    expect(listPosts).toHaveBeenCalledWith(true)
    expect(row(1).text()).toContain('Draft')
    expect(row(2).text()).toContain('Scheduled')
    expect(row(3).text()).toContain('Published')
    expect(row(3).get('.badge').text()).toBe('7')
  })

  it('publishes a draft as of now and unpublishes a published post', async () => {
    setUser(ADMIN)
    vi.mocked(setPostPublication).mockResolvedValue(makePost())
    await open()

    const before = Date.now()
    await row(1).get('button[title="Publish now"]').trigger('click')
    await flushPromises()
    const published = vi.mocked(setPostPublication).mock.calls[0]
    expect(published[0]).toBe(1)
    expect(new Date(published[1]!).getTime()).toBeGreaterThanOrEqual(before)
    expect(new Date(published[1]!).getTime()).toBeLessThanOrEqual(Date.now())

    expect(row(3).find('button[title="Publish now"]').exists()).toBe(false)
    await row(3).get('button[title="Unpublish"]').trigger('click')
    await flushPromises()
    expect(setPostPublication).toHaveBeenLastCalledWith(3, null)
    // The list is reloaded after each change
    expect(listPosts).toHaveBeenCalledTimes(3)
  })

  it('deletes a post once confirmed', async () => {
    setUser(ADMIN)
    vi.mocked(deletePost).mockResolvedValue(undefined)
    await open()

    const shown = modalEvent('shown.bs.modal')
    await row(3).get('button[title="Delete post"]').trigger('click')
    await shown
    const modal = wrapper!.getComponent(ConfirmModal)
    expect(modal.text()).toContain('Read by 7 member(s)')
    expect(deletePost).not.toHaveBeenCalled()

    const hidden = modalEvent('hidden.bs.modal')
    await modal.findAll('button').find((button) => button.text() === 'Delete')!.trigger('click')
    await hidden
    await flushPromises()
    expect(deletePost).toHaveBeenCalledWith(3)
  })

  it('is refused to members without the editor role', async () => {
    setUser(MEMBER)
    await open()
    expect(wrapper!.text()).toContain('needs the admin or editor role')
    expect(listPosts).not.toHaveBeenCalled()
  })
})
