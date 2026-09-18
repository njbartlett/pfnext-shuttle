<template>
  <span class="text-nowrap" :title="rounded + ' / 5 stars'">
    <i v-for="star in stars" :key="star.index" class="bi" :class="star.icon"></i>
  </span>
</template>

<script setup lang="ts">
// Five-star display of an average rating, with half stars
import { computed } from 'vue'

const props = defineProps<{ rating: number }>()

const rounded = computed(() => Math.round(props.rating * 10) / 10)

const stars = computed(() =>
  Array.from({ length: 5 }, (_, index) => {
    const remaining = props.rating - index
    let icon = 'bi-star'
    if (remaining >= 0.8) {
      icon = 'bi-star-fill'
    } else if (remaining >= 0.2) {
      icon = 'bi-star-half'
    }
    return { index, icon }
  })
)
</script>
