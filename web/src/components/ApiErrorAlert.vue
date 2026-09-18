<template>
  <div v-if="apiError" class="alert alert-danger my-2 text-center" role="alert">
    <i class="bi bi-exclamation-triangle-fill"></i>&nbsp;{{ apiError.message }}
  </div>
</template>

<script setup lang="ts">
import { apiError } from '@/stores/apiError'
</script>
