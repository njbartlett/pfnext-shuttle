<template>
  <div v-if="isAdmin" class="alert alert-warning my-3 position-relative">
    <div class="position-absolute top-0 end-0"><i class="bi bi-lightning-fill" title="Admin Only"></i></div>
    <slot />
  </div>
</template>

<script setup lang="ts">
// The yellow "admin only" panel; renders nothing for other users
import { isAdmin } from '@/stores/auth'
</script>
