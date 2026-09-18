<template>
  <img v-if="status === 'green'" class="icon" src="/img/bib-green.svg" :alt="alt ?? 'Green Bib'" title="Green Bib Member">
  <img v-else-if="status === 'red'" class="icon" src="/img/bib-red.svg" :alt="alt ?? 'Red Bib'" title="Red Bib Member">
  <img v-else-if="status === 'blue'" class="icon" src="/img/bib-blue.svg" :alt="alt ?? 'Blue Bib'" title="Blue Bib Member">
  <slot v-else />
</template>

<script setup lang="ts">
// Member bib colour marker. The slot renders when the member has no bib.
defineProps<{ status: string | null | undefined; alt?: string }>()
</script>
