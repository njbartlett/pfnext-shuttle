<template>
  <!-- A full-height column so the footer follows the content and, on a page
       shorter than the window, still sits at the bottom -->
  <div class="d-flex flex-column min-vh-100">
    <template v-if="showNavbar">
      <AppNavbar />
      <ApiErrorAlert />
      <RouterView />
    </template>
    <!-- Focused pages (login, editors) render without the navbar, inside a
         plain container -->
    <div v-else class="container">
      <ApiErrorAlert />
      <RouterView />
    </div>

    <footer class="mt-auto pt-5 mx-2 text-secondary d-print-none">
      <p>&copy; 2024 &ndash; 2026 Neil Bartlett all rights reserved.</p>
    </footer>
  </div>
</template>

<script setup lang="ts">
// Application shell: navbar, API error banner, the routed view and footer
import { computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import AppNavbar from '@/components/AppNavbar.vue'
import ApiErrorAlert from '@/components/ApiErrorAlert.vue'
import { loginRoute } from '@/router'
import { isLoggedIn } from '@/stores/auth'

const route = useRoute()
const router = useRouter()
const showNavbar = computed(() => route.meta.navbar !== false)

// A login that expires or is revoked while on a member page (the API answered
// 401 and the auth store cleared the user) sends the user to log in again
watch(isLoggedIn, (loggedIn) => {
  if (!loggedIn && route.meta.requiresLogin) {
    void router.replace(loginRoute(route.fullPath))
  }
})
</script>
