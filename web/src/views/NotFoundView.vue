<template>
  <div class="container">
    <PageTitle />
    <p class="fs-5 text-body-secondary text-center">The requested page could not be found :-(</p>
  </div>
</template>

<script setup lang="ts">
// Catch-all route; replaces templates/404.html.tera
import PageTitle from '@/components/PageTitle.vue'
</script>
