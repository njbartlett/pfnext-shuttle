<template>
  <div class="container">
    <PageTitle />

    <!-- Editor tools -->
    <div v-if="isEditor" class="alert alert-warning my-3 position-relative">
      <div class="position-absolute top-0 end-0"><i class="bi bi-lightning-fill" title="Admin/Editor Only"></i></div>
      <div class="d-flex">
        <RouterLink class="btn btn-primary btn-sm mx-1" :to="{ name: 'edit_post', params: { location: 'new' } }"><i class="bi bi-feather"></i>&nbsp;New Post</RouterLink>
        <RouterLink v-if="showAll" class="btn btn-outline-primary btn-sm mx-1" :to="{ name: 'blog' }">Show Only Published</RouterLink>
        <RouterLink v-else class="btn btn-outline-primary btn-sm mx-1" :to="{ name: 'blog', query: { mode: ALL_POSTS_MODE } }">Show All</RouterLink>
      </div>
      <div v-if="showAll" class="mt-3">Showing all published and unpublished posts.</div>
      <div v-else class="mt-3">Showing published posts as of {{ displayDayMonthYear(new Date()) }}.</div>
    </div>

    <ul v-if="posts.length > 0" class="list-group">
      <li v-for="post in posts" :key="post.id" class="list-group-item d-flex justify-content-between align-items-start">
        <div class="ms-2 me-auto">
          <div class="fw-bold fs-5">
            <RouterLink :to="{ name: 'blog_post', params: { location: postLocation(post.title) } }" class="stretched-link">{{ post.title }}</RouterLink>
          </div>
          By {{ post.author_name }}
          <span v-if="post.published_at" class="fst-italic">on {{ displayDayMonthYear(post.published_at) }}</span>
          <span v-else class="fst-italic">(unpublished)</span>.
        </div>
      </li>
    </ul>
    <p v-else-if="loaded">There are no posts currently available.</p>
  </div>
</template>

<script setup lang="ts">
// List of blog posts; editors can include drafts with ?mode=allposts
import { computed, ref, watch } from 'vue'
import { displayDayMonthYear, listPosts, postLocation, type PostSummary } from '@pfnext/shared'
import PageTitle from '@/components/PageTitle.vue'
import { useQueryState } from '@/composables/useQueryState'
import { isEditor } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const ALL_POSTS_MODE = 'allposts'

const query = useQueryState()
const posts = ref<PostSummary[]>([])
const loaded = ref(false)

// Only editors may list unpublished posts; for anyone else the mode is ignored
const showAll = computed(() => isEditor.value && query.get('mode') === ALL_POSTS_MODE)

async function load() {
  const result = await tryApi(() => listPosts(showAll.value))
  if (result) {
    posts.value = result
  }
  loaded.value = true
}

watch(showAll, load, { immediate: true })
</script>
