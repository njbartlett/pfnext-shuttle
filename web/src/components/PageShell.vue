<template>
  <template v-if="navbar">
    <AppNavbar />
    <ApiErrorAlert />
    <component :is="view" />
  </template>
  <!-- Without a navbar, the shell provides the container the legacy
       nonav_layout wrapped the page in -->
  <div v-else class="container">
    <ApiErrorAlert />
    <component :is="view" />
  </div>
</template>

<script setup lang="ts">
// Common frame for every module page: what templates/default_layout.html.tera
// and templates/navigation.html.tera provide for the legacy pages.
import type { Component } from 'vue'
import AppNavbar from './AppNavbar.vue'
import ApiErrorAlert from './ApiErrorAlert.vue'

defineProps<{ view: Component; navbar: boolean }>()
</script>
