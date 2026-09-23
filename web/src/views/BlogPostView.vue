<template>
  <div class="container">
    <template v-if="post">
      <PageTitle :title="post.title" />

      <div class="text-center mt-2 mx-1">
        <span class="text-body-secondary">
          Created by {{ post.author_name }} <span class="fst-italic">on {{ displayDayMonthYear(post.created_at) }}</span>,
          last edited by {{ post.last_editor_name }} <span class="fst-italic">on {{ displayDayMonthYear(post.last_edited_at) }}</span>.
        </span>
        <br>
        <span v-if="post.published_at" class="text-body-secondary">
          Published <span class="fst-italic">on {{ displayDayMonthYear(post.published_at) }}</span>.
        </span>
        <span v-else class="text-danger"><i class="bi bi-exclamation-circle"></i> Unpublished Draft</span>
      </div>

      <!-- Editor tools -->
      <div v-if="isEditor" class="admin-panel my-3">
        <div class="admin-panel-icon"><i class="bi bi-lightning-fill" title="Admin/Editor Only"></i></div>
        <div class="d-flex">
          <RouterLink class="btn btn-primary btn-sm ms-auto" :to="{ name: 'edit_post', params: { location } }"><i class="bi bi-feather"></i>&nbsp;Edit Post</RouterLink>
          <button type="button" class="btn btn-danger btn-sm ms-1" @click="deleteModal?.show()"><i class="bi bi-trash"></i> Delete</button>
        </div>
      </div>

      <div class="d-flex my-3">
        <RouterLink :to="{ name: 'blog' }">&laquo; Return to List of Posts</RouterLink>
      </div>

      <!-- Post bodies are HTML written by editors -->
      <div class="post-content" v-html="post.content"></div>

      <ConfirmModal ref="deleteModal" title="Delete Post" icon="bi-exclamation-triangle-fill" header-class="text-bg-danger" confirm-label="Delete" confirm-icon="bi-trash" confirm-class="btn-danger" @confirm="confirmedDelete">
        <p>Are you sure you want to delete this Post?</p>
        <p>This cannot be undone!</p>
      </ConfirmModal>
    </template>

    <template v-else-if="missing">
      <PageTitle title="Post Not Found" />
      <p class="text-center">There is no post called &ldquo;{{ title }}&rdquo;.</p>
      <div class="d-flex my-3 justify-content-center">
        <RouterLink :to="{ name: 'blog' }">&laquo; Return to List of Posts</RouterLink>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
// One blog post, addressed by its title in the URL. Rocket adds the post's
// Open Graph tags to the page shell (src/blog.rs) for link previews; the
// page itself is rendered here.
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { ApiError, deletePost, displayDayMonthYear, getPostByTitle, postTitle, type Post } from '@pfnext/shared'
import ConfirmModal from '@/components/ConfirmModal.vue'
import PageTitle from '@/components/PageTitle.vue'
import { SITE_NAME } from '@/app/site'
import { isEditor } from '@/stores/auth'
import { reportApiError, tryApi } from '@/stores/apiError'

const route = useRoute()
const router = useRouter()

const location = computed(() => String(route.params.location))
const title = computed(() => postTitle(location.value) ?? location.value)
const post = ref<Post | null>(null)
const missing = ref(false)
const deleteModal = ref<InstanceType<typeof ConfirmModal> | null>(null)

async function load() {
  missing.value = false
  const postTitleValue = postTitle(location.value)
  if (postTitleValue === null) {
    post.value = null
    missing.value = true
    return
  }
  try {
    post.value = await getPostByTitle(postTitleValue)
    document.title = `${post.value.title} – ${SITE_NAME}`
  } catch (error) {
    post.value = null
    if (error instanceof ApiError && error.status === 404) {
      missing.value = true
    } else {
      reportApiError(error)
    }
  }
}

async function confirmedDelete() {
  if (!post.value) {
    return
  }
  const done = await tryApi(async () => {
    await deletePost(post.value!.id)
    return true
  })
  if (done) {
    void router.push({ name: 'blog', query: { mode: 'allposts' } })
  }
}

// The same component serves every post, so reload when the URL changes
watch(location, load, { immediate: true })
</script>

<style>
/* Images in post bodies come in at their stored size */
.post-content img {
  max-width: 100%;
  height: auto;
}
</style>
