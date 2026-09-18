<template>
  <div class="mt-2 mx-auto text-center">
    <h1 class="display-4 fw-normal text-body-emphasis">{{ title ?? page.title }}</h1>
  </div>
</template>

<script setup lang="ts">
// The page heading; defaults to the title from templates/pages.toml
import { usePageContext } from '@/app/pageContext'

defineProps<{ title?: string }>()
const { page } = usePageContext()
</script>
