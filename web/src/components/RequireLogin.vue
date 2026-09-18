<template>
  <div v-if="!isLoggedIn" class="alert alert-info my-2 text-center align-middle" role="alert">
    <i class="bi bi-info-circle"></i>&nbsp;You must log in to continue&nbsp;
    <a class="btn btn-success rounded-pill my-1 mx-2" :href="'/login.html?return=' + loginReturnUrl()">Login</a>
  </div>
  <slot v-else />
</template>

<script setup lang="ts">
// Gate for member-only pages; replaces templates/loggedin_layout.html.tera
import { isLoggedIn, loginReturnUrl } from '@/stores/auth'
</script>
