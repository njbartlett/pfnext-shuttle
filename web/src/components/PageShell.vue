<template>
  <AppNavbar />
  <ApiErrorAlert />
  <component :is="view" />
</template>

<script setup lang="ts">
// Common frame for every module page: what templates/default_layout.html.tera
// and templates/navigation.html.tera provide for the legacy pages.
import type { Component } from 'vue'
import AppNavbar from './AppNavbar.vue'
import ApiErrorAlert from './ApiErrorAlert.vue'

defineProps<{ view: Component }>()
</script>
