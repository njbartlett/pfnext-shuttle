<template>
  <a id="bd-theme" href="#" role="button" class="nav-link dropdown-toggle navbar-text px-0 px-lg-2" data-bs-toggle="dropdown" aria-expanded="false" :aria-label="'Toggle Theme (' + theme + ')'">
    <i :class="ICONS[theme]"></i><span class="d-lg-none ms-2">Toggle Theme</span>
  </a>
  <ul class="dropdown-menu dropdown-menu-lg-end">
    <li v-for="option in THEMES" :key="option">
      <a class="dropdown-item" href="#" :class="{ active: option === theme }" :aria-pressed="option === theme" @click.prevent="setTheme(option)">
        <i :class="ICONS[option]"></i>&nbsp;{{ LABELS[option] }}
      </a>
    </li>
  </ul>
</template>

<script setup lang="ts">
// Navbar dropdown for the colour theme; replaces static/js/theme.js
import { useTheme, type Theme } from '@/composables/useTheme'

const THEMES: Theme[] = ['dark', 'light']
const ICONS: Record<Theme, string> = { dark: 'bi bi-moon-fill', light: 'bi bi-sun-fill' }
const LABELS: Record<Theme, string> = { dark: 'Dark', light: 'Light' }

const { theme, setTheme } = useTheme()
</script>
