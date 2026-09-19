<template>
  <div class="mt-2 mx-auto text-center">
    <h1 class="display-4 fw-normal text-body-emphasis">{{ title ?? route.meta.title }}</h1>
  </div>
</template>

<script setup lang="ts">
// The page heading; defaults to the route's title
import { useRoute } from 'vue-router'

defineProps<{ title?: string }>()
const route = useRoute()
</script>
