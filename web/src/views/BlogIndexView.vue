<template>
  <div class="container">
    <PageTitle />

    <!-- Editor tools -->
    <div v-if="isEditor" class="admin-panel my-3">
      <div class="admin-panel-icon"><i class="bi bi-lightning-fill" title="Admin/Editor Only"></i></div>
      <div class="d-flex">
        <RouterLink class="btn btn-primary btn-sm mx-1" :to="{ name: 'edit_post', params: { location: 'new' } }"><i class="bi bi-feather"></i>&nbsp;New Post</RouterLink>
        <RouterLink class="btn btn-outline-primary btn-sm mx-1" :to="{ name: 'blog_admin' }"><i class="bi bi-list-check"></i>&nbsp;Manage Posts</RouterLink>
      </div>
    </div>

    <ul v-if="posts.length > 0" class="list-group">
      <li v-for="post in posts" :key="post.id" class="list-group-item d-flex justify-content-between align-items-start">
        <div class="ms-2 me-auto">
          <div class="fw-bold fs-5">
            <RouterLink :to="{ name: 'blog_post', params: { location: postLocation(post.title) } }" class="stretched-link">{{ post.title }}</RouterLink>
            <span v-if="isUnread(post)" class="badge rounded-pill text-bg-primary align-middle ms-2 fs-6">New</span>
          </div>
          By {{ post.author_name }} <span v-if="post.published_at" class="fst-italic">on {{ displayDayMonthYear(post.published_at) }}</span>.
        </div>
      </li>
    </ul>
    <p v-else-if="loaded">There are no posts currently available.</p>
  </div>
</template>

<script setup lang="ts">
// List of published blog posts. Editors manage all posts, drafts included,
// on the blog admin page.
import { ref, watch } from 'vue'
import { displayDayMonthYear, listPosts, postLocation, type PostSummary } from '@pfnext/shared'
import PageTitle from '@/components/PageTitle.vue'
import { isEditor, isLoggedIn } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const posts = ref<PostSummary[]>([])
const loaded = ref(false)

// Only logged-in users' reads are tracked
function isUnread(post: PostSummary): boolean {
  return isLoggedIn.value && post.read_at === null
}

async function load() {
  const result = await tryApi(() => listPosts())
  if (result) {
    posts.value = result
  }
  loaded.value = true
}

// Reload on login and logout too: read times are per user
watch(isLoggedIn, load, { immediate: true })
</script>
