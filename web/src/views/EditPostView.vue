<template>
  <div class="container">
    <PageTitle :title="pageTitle" />

    <div v-if="!isEditor" class="alert alert-warning" role="alert">
      <i class="bi bi-exclamation-triangle-fill"></i>&nbsp;Writing posts needs the admin or editor role.
    </div>

    <template v-else-if="missing">
      <p class="text-center">There is no post called &ldquo;{{ postTitle(location) }}&rdquo;.</p>
      <div class="d-flex my-3 justify-content-center">
        <RouterLink :to="{ name: 'blog' }">&laquo; Return to List of Posts</RouterLink>
      </div>
    </template>

    <template v-else-if="loaded">
      <div class="card my-2 border border-primary">
        <div class="card-header d-flex pe-2 align-items-center">
          <span class="fs-5">Post Info</span>
          <div class="ms-auto"><span v-if="status" class="fst-italic text-success">{{ status }}</span></div>
          <button type="button" class="btn btn-sm btn-primary ms-2" :disabled="!dirty || !valid" @click="save"><i class="bi bi-floppy"></i> Save</button>
          <button type="button" class="btn btn-sm btn-outline-danger ms-1" @click="close">Close</button>
        </div>
        <div class="card-body">
          <form @submit.prevent="save">
            <div class="row">
              <div class="col-sm-6">
                <label for="postTitle" class="form-label">Title</label>
                <input id="postTitle" v-model="form.title" type="text" class="form-control" :class="{ 'is-invalid': errors.title }">
                <div class="invalid-feedback"><i class="bi bi-exclamation-circle"></i> {{ errors.title }}</div>
              </div>
              <div class="col-sm-3">
                <label for="postPubDate" class="form-label">Publication Date</label>
                <input id="postPubDate" v-model="form.pubDate" class="form-control" type="date" aria-describedby="postPubHelp">
              </div>
              <div class="col-sm-3">
                <label for="postPubTime" class="form-label">Publication Time</label>
                <input id="postPubTime" v-model="form.pubTime" class="form-control" type="time" :class="{ 'is-invalid': errors.pubTime }">
                <div class="invalid-feedback"><i class="bi bi-exclamation-circle"></i> {{ errors.pubTime }}</div>
              </div>
            </div>
            <div class="row">
              <div class="col-sm-6"></div>
              <div id="postPubHelp" class="form-text col-sm-6"><i class="bi bi-info-circle"></i> When publication date is unspecified, the post is left unpublished.</div>
            </div>
          </form>
        </div>
      </div>

      <div class="card my-2 border border-secondary">
        <div class="card-header d-flex align-items-center">
          <span class="fs-5">Post Content</span>
        </div>
        <!-- Re-created when the colour theme changes, so the editor re-skins -->
        <Editor :key="theme" v-model="form.content" :init="editorInit" license-key="gpl" />
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
// Writes and edits blog posts with TinyMCE (self-hosted from npm). The URL
// is /blog/edit/new or /blog/edit/<title>.html; after the first save the
// editor moves to the saved post's URL.
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
import Editor from '@tinymce/tinymce-vue'
import 'tinymce/tinymce'
import 'tinymce/icons/default'
import 'tinymce/themes/silver'
import 'tinymce/models/dom'
import 'tinymce/plugins/advlist'
import 'tinymce/plugins/image'
import 'tinymce/plugins/link'
import 'tinymce/plugins/lists'
import {
  ApiError, apiBaseUrl, createPost, getPostByTitle, postLocation, postTitle, updatePost, type Post, type SavePost
} from '@pfnext/shared'
import PageTitle from '@/components/PageTitle.vue'
import { useDirtyTracking } from '@/composables/useDirtyTracking'
import { useTheme } from '@/composables/useTheme'
import { SITE_NAME } from '@/app/site'
import { isEditor } from '@/stores/auth'
import { reportApiError, tryApi } from '@/stores/apiError'

const NEW_POST = 'new'
// Where vite.config.ts copies TinyMCE's skins to
const TINYMCE_SKINS_URL = '/tinymce/skins'

const route = useRoute()
const router = useRouter()
const { theme } = useTheme()

const location = computed(() => String(route.params.location))
// The post as last saved; null while creating a new one
const post = ref<Post | null>(null)
const form = reactive({ title: '', pubDate: '', pubTime: '', content: '' })
const { dirty, markClean } = useDirtyTracking(form)
const status = ref<string | null>(null)
const loaded = ref(false)
const missing = ref(false)

const pageTitle = computed(() => (post.value ? `Edit Post – ${post.value.title}` : 'Create Post'))

const errors = computed(() => ({
  title: form.title.trim() ? null : 'Title must not be empty.',
  pubTime: form.pubDate && !form.pubTime ? 'Publication time must be specified.' : null
}))
const valid = computed(() => Object.values(errors.value).every((error) => error === null))

const editorInit = computed(() => {
  const dark = theme.value === 'dark'
  return {
    plugins: 'advlist link image lists',
    menubar: 'edit view insert format',
    placeholder: 'Type here...',
    height: 500,
    promotion: false,
    // Images are uploaded to the API as they are inserted, then referenced
    // by URL in the post body
    images_upload_url: `${apiBaseUrl()}/blobs`,
    images_upload_credentials: true,
    skin_url: `${TINYMCE_SKINS_URL}/ui/${dark ? 'oxide-dark' : 'oxide'}`,
    content_css: `${TINYMCE_SKINS_URL}/content/${dark ? 'dark' : 'default'}/content.css`
  }
})

function pad(value: number): string {
  return String(value).padStart(2, '0')
}

function fillFrom(saved: Post | null) {
  post.value = saved
  form.title = saved?.title ?? ''
  form.content = saved?.content ?? ''
  if (saved?.published_at) {
    const published = new Date(saved.published_at)
    form.pubDate = `${published.getFullYear()}-${pad(published.getMonth() + 1)}-${pad(published.getDate())}`
    form.pubTime = `${pad(published.getHours())}:${pad(published.getMinutes())}`
  } else {
    form.pubDate = ''
    form.pubTime = ''
  }
  markClean()
  document.title = `${pageTitle.value} – ${SITE_NAME}`
}

async function load() {
  missing.value = false
  if (location.value === NEW_POST) {
    fillFrom(null)
  } else {
    const title = postTitle(location.value)
    if (title === null) {
      missing.value = true
      return
    }
    try {
      fillFrom(await getPostByTitle(title))
    } catch (error) {
      if (error instanceof ApiError && error.status === 404) {
        missing.value = true
      } else {
        reportApiError(error)
      }
      return
    }
  }
  loaded.value = true
}

function toRequest(): SavePost {
  return {
    title: form.title.trim(),
    content: form.content,
    published: form.pubDate ? new Date(`${form.pubDate}T${form.pubTime || '00:00'}`).toISOString() : null
  }
}

async function save() {
  if (!valid.value) {
    return
  }
  const request = toRequest()
  const saved = await tryApi(() => (post.value ? updatePost(post.value.id, request) : createPost(request)))
  if (!saved) {
    return
  }
  fillFrom(saved)
  status.value = `Saved at ${new Date().toLocaleTimeString('en-GB')}`
  // The URL names the post by title, so follow a new or renamed post
  const savedLocation = postLocation(saved.title)
  if (savedLocation !== location.value) {
    await router.replace({ name: 'edit_post', params: { location: savedLocation } })
  }
}

function close() {
  void router.push(post.value ? { name: 'blog_post', params: { location: postLocation(post.value.title) } } : { name: 'blog' })
}

// Reload when the URL changes to a different post; a save that moved the
// URL to the post just saved needs no reload
watch(location, () => {
  if (!post.value || postLocation(post.value.title) !== location.value) {
    void load()
  }
})
void load()

watch(dirty, (isDirty) => {
  if (isDirty) {
    status.value = null
  }
})

// Unsaved changes: warn on leaving the page or the site
onBeforeRouteLeave(() => {
  if (dirty.value && !window.confirm('Discard unsaved changes to this post?')) {
    return false
  }
  return true
})
const onBeforeUnload = (event: BeforeUnloadEvent) => {
  if (dirty.value) {
    event.preventDefault()
  }
}
window.addEventListener('beforeunload', onBeforeUnload)
onBeforeUnmount(() => window.removeEventListener('beforeunload', onBeforeUnload))
</script>

<style>
/* The editor fills its card */
.tox-tinymce {
  border: 0 !important;
  border-radius: 0 !important;
}
</style>
