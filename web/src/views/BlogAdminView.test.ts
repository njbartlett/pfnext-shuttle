// The editors' list of all posts: drafts, scheduled and published posts with
// read counts, and the publish/unpublish and delete actions; and the panel
// of uploaded images that no post embeds. API calls are mocked.
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, type VueWrapper } from '@vue/test-utils'
import {
  deleteAllOrphanImages, deleteOrphanImage, deletePost, listOrphanImages, listPosts, setPostPublication, type OrphanImage
} from '@pfnext/shared'
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

const IMAGES: OrphanImage[] = [
  { name: 'aB3xK9pQ2mZt.jpg', url: 'https://b.lon1.cdn.digitaloceanspaces.com/pfnext/blog/images/aB3xK9pQ2mZt.jpg', size: 150_000, uploaded_at: '2025-06-01T09:00:00Z' },
  { name: 'Zt9pQ2mB3xKa.png', url: 'https://b.lon1.cdn.digitaloceanspaces.com/pfnext/blog/images/Zt9pQ2mB3xKa.png', size: 3_000_000, uploaded_at: null }
]

let wrapper: VueWrapper | undefined

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(listPosts).mockResolvedValue(POSTS)
  vi.mocked(listOrphanImages).mockResolvedValue(IMAGES)
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
    expect(listOrphanImages).not.toHaveBeenCalled()
  })
})

describe('the orphaned images panel', () => {
  function imageRow(name: string) {
    return wrapper!.get(`[id="image-${name}"]`)
  }

  it('links to each image in a new tab without showing it', async () => {
    setUser(ADMIN)
    await open()
    const link = imageRow('aB3xK9pQ2mZt.jpg').get('a')
    expect(link.attributes('href')).toBe(IMAGES[0].url)
    expect(link.attributes('target')).toBe('_blank')
    expect(imageRow('aB3xK9pQ2mZt.jpg').text()).toContain('147 KiB')
    expect(imageRow('Zt9pQ2mB3xKa.png').text()).toContain('2.9 MiB')
    expect(wrapper!.find('img').exists()).toBe(false)
    expect(wrapper!.get('[role="note"]').text()).toContain('new posts that have not been saved')
  })

  it('deletes one image and reloads the list', async () => {
    setUser(ADMIN)
    vi.mocked(deleteOrphanImage).mockResolvedValue(undefined)
    await open()
    await imageRow('Zt9pQ2mB3xKa.png').get('button').trigger('click')
    await flushPromises()
    expect(deleteOrphanImage).toHaveBeenCalledWith('Zt9pQ2mB3xKa.png')
    expect(listOrphanImages).toHaveBeenCalledTimes(2)
  })

  it('deletes all images once confirmed', async () => {
    setUser(ADMIN)
    vi.mocked(deleteAllOrphanImages).mockResolvedValue(undefined)
    await open()

    const shown = modalEvent('shown.bs.modal')
    await wrapper!.findAll('button').find((button) => button.text() === 'Delete All')!.trigger('click')
    await shown
    const modal = wrapper!.findAllComponents(ConfirmModal).find((m) => m.text().includes('orphaned image(s)'))!
    expect(modal.text()).toContain('delete all 2 orphaned image(s)')
    expect(deleteAllOrphanImages).not.toHaveBeenCalled()

    const hidden = modalEvent('hidden.bs.modal')
    await modal.findAll('button').find((button) => button.text() === 'Delete All')!.trigger('click')
    await hidden
    await flushPromises()
    expect(deleteAllOrphanImages).toHaveBeenCalled()
  })

  it('says so when every image is in use', async () => {
    setUser(ADMIN)
    vi.mocked(listOrphanImages).mockResolvedValue([])
    await open()
    expect(wrapper!.text()).toContain('Every uploaded image is used by a post.')
    // Only the (hidden) confirmation modal has a Delete All button
    expect(wrapper!.findAll('.card button').some((button) => button.text() === 'Delete All')).toBe(false)
  })
})
