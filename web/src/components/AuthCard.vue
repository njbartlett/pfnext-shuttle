<template>
  <div class="card my-2">
    <div class="card-header d-flex align-items-center">
      <svg xmlns="http://www.w3.org/2000/svg" width="50" height="50" fill="currentColor" class="bi bi-sun-fill" viewBox="0 0 54 54">
        <path d="m 29.64,32.02 2.7,-4.68 8.16,14.04 h 5.4 C 44.7,39.46 33.3,19.96 32.34,18.04 L 20.58,38.44 16.2,46.06 H 0 C 0.9,44.56 6.96,34 12.78,23.86 16.2,17.92 19.98,11.26 24.24,4 h 5.4 L 18.48,23.32 8.1,41.38 h 5.4 C 20.88,28.36 25.5,20.68 32.34,8.68 32.76,9.4 53.88,46 54,46.06 H 37.8 Z"/>
      </svg>
      <h2 class="ms-2">{{ SITE_NAME }} &ndash; {{ title }}</h2>
      <div class="ms-auto align-middle">
        <button type="button" class="btn-close" aria-label="Close" @click="goBack()"></button>
      </div>
    </div>
    <div class="card-body">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
// Branded card used by the login, registration and password reset pages
import { SITE_NAME } from '@/app/site'
import { useReturnPath } from '@/composables/useReturnPath'

defineProps<{ title: string }>()
const { goBack } = useReturnPath()
</script>
