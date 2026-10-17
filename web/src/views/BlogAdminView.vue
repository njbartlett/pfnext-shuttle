<template>
  <div class="container">
    <PageTitle />

    <div v-if="!isEditor" class="alert alert-warning my-3" role="alert">
      <i class="bi bi-exclamation-triangle-fill"></i>&nbsp;Managing posts needs the admin or editor role.
    </div>

    <div v-else class="card my-3">
      <h5 class="card-header d-flex align-items-center">
        <span>All Posts</span>
        <RouterLink class="btn btn-outline-primary btn-sm ms-auto" :to="{ name: 'blog' }">&laquo; Published Posts</RouterLink>
        <RouterLink class="btn btn-primary btn-sm ms-1" :to="{ name: 'edit_post', params: { location: 'new' } }"><i class="bi bi-feather"></i>&nbsp;New Post</RouterLink>
      </h5>
      <div class="card-body">
        <div v-if="loaded && posts.length === 0" class="alert alert-secondary mb-0" role="alert">
          <em>No posts have been written yet.</em>
        </div>
        <div v-else class="table-responsive">
          <table class="table table-hover table-sm align-middle mb-0">
            <thead>
              <tr>
                <th scope="col">Title</th>
                <th scope="col">Status</th>
                <th scope="col">Author</th>
                <th scope="col"></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="post in posts" :id="'post-' + post.id" :key="post.id">
                <td>
                  <RouterLink :to="{ name: 'blog_post', params: { location: postLocation(post.title) } }">{{ post.title }}</RouterLink>
                  <span class="badge rounded-pill text-bg-secondary ms-2" :title="readCountTitle(post)"><i class="bi bi-eye"></i>&nbsp;{{ post.read_count ?? 0 }}</span>
                </td>
                <td class="text-nowrap">
                  <span class="badge rounded-pill" :class="STATUS_BADGES[statusOf(post)]">{{ STATUS_LABELS[statusOf(post)] }}</span>
                  <span v-if="post.published_at" class="small text-secondary ms-1">{{ displayDateTime(post.published_at) }}</span>
                </td>
                <td>{{ post.author_name }}</td>
                <td class="text-end text-nowrap">
                  <div class="btn-group btn-group-sm" role="group">
                    <button v-if="statusOf(post) !== 'published'" type="button" class="btn btn-outline-success" title="Publish now" @click="setPublished(post, true)"><i class="bi bi-send-fill"></i></button>
                    <button v-if="statusOf(post) !== 'draft'" type="button" class="btn btn-outline-secondary" title="Unpublish" @click="setPublished(post, false)"><i class="bi bi-eye-slash-fill"></i></button>
                    <RouterLink class="btn btn-outline-primary" title="Edit post" :to="{ name: 'edit_post', params: { location: postLocation(post.title) } }"><i class="bi bi-pencil"></i></RouterLink>
                    <button type="button" class="btn btn-outline-danger" title="Delete post" @click="askDelete(post)"><i class="bi bi-trash-fill"></i></button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
      <div class="card-footer text-secondary small">
        Publishing sets the publication time to now. Scheduled posts appear on the blog at their publication time. Deleting a post also deletes its read records.
      </div>
    </div>

    <ConfirmModal ref="deleteModal" title="Delete Post" icon="bi-exclamation-triangle-fill" header-class="text-bg-danger" confirm-label="Delete" confirm-icon="bi-trash" confirm-class="btn-danger" @confirm="confirmDelete">
      <template v-if="deleting">
        <p>Are you sure you want to delete the following post?</p>
        <ul>
          <li><strong>{{ deleting.title }}</strong></li>
          <li>{{ STATUS_LABELS[statusOf(deleting)] }}</li>
          <li>Read by {{ deleting.read_count ?? 0 }} member(s)</li>
        </ul>
        <p>This cannot be undone.</p>
      </template>
    </ConfirmModal>
  </div>
</template>

<script setup lang="ts">
// The editors' view of the blog: every post, drafts and scheduled posts
// included, with read counts and per-post publish, edit and delete actions
import { ref, watch } from 'vue'
import { deletePost, displayDateTime, isPast, listPosts, postLocation, setPostPublication, type PostSummary } from '@pfnext/shared'
import ConfirmModal from '@/components/ConfirmModal.vue'
import PageTitle from '@/components/PageTitle.vue'
import { isEditor } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

type Status = 'draft' | 'scheduled' | 'published'

const STATUS_LABELS: Record<Status, string> = { draft: 'Draft', scheduled: 'Scheduled', published: 'Published' }
const STATUS_BADGES: Record<Status, string> = { draft: 'text-bg-secondary', scheduled: 'text-bg-warning', published: 'text-bg-primary' }

const posts = ref<PostSummary[]>([])
const loaded = ref(false)
const deleting = ref<PostSummary | null>(null)
const deleteModal = ref<InstanceType<typeof ConfirmModal> | null>(null)

function statusOf(post: PostSummary): Status {
  if (post.published_at === null) {
    return 'draft'
  }
  return isPast(post.published_at) ? 'published' : 'scheduled'
}

function readCountTitle(post: PostSummary): string {
  const count = post.read_count ?? 0
  return `Read by ${count} member${count === 1 ? '' : 's'}`
}

async function load() {
  if (!isEditor.value) {
    return
  }
  const result = await tryApi(() => listPosts(true))
  if (result) {
    posts.value = result
  }
  loaded.value = true
}

async function setPublished(post: PostSummary, publish: boolean) {
  const published = publish ? new Date().toISOString() : null
  if (await tryApi(() => setPostPublication(post.id, published))) {
    await load()
  }
}

function askDelete(post: PostSummary) {
  deleting.value = post
  deleteModal.value?.show()
}

async function confirmDelete() {
  if (!deleting.value) {
    return
  }
  const postId = deleting.value.id
  const done = await tryApi(async () => {
    await deletePost(postId)
    return true
  })
  if (done) {
    deleting.value = null
    await load()
  }
}

watch(isEditor, load, { immediate: true })
</script>
