<template>
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

  <div class="sticky-bottom bg-body mt-5 mx-2 text-secondary d-print-none z-n1">
    <p>&copy; 2024 &ndash; 2025 Neil Bartlett all rights reserved.</p>
  </div>
</template>

<script setup lang="ts">
// Application shell: navbar, API error banner, the routed view and footer
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import AppNavbar from '@/components/AppNavbar.vue'
import ApiErrorAlert from '@/components/ApiErrorAlert.vue'

const route = useRoute()
const showNavbar = computed(() => route.meta.navbar !== false)
</script>
