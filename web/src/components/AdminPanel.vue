<template>
  <div v-if="isAdmin" class="admin-panel my-3">
    <div class="admin-panel-icon"><i class="bi bi-lightning-fill" title="Admin Only"></i></div>
    <slot />
  </div>
</template>

<script setup lang="ts">
// The "admin only" tool panel; renders nothing for other users
import { isAdmin } from '@/stores/auth'
</script>
