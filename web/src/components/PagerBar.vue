<template>
  <div class="my-2 d-flex flex-wrap">
    <div class="btn-group my-1 flex-grow-1" role="group" style="width: max-content">
      <button type="button" class="btn btn-outline-primary flex-grow-0" @click="emit('back')">&laquo;</button>
      <button
        v-for="index in indices"
        :key="index"
        type="button"
        class="btn"
        :class="index === offset ? 'btn-primary' : 'btn-outline-primary'"
        @click.stop="emit('select', index)"
      >
        {{ label(index) }}
      </button>
      <button type="button" class="btn btn-outline-primary flex-grow-0" @click="emit('forward')">&raquo;</button>
    </div>
    <div class="my-1 ms-auto">
      <button type="button" class="btn btn-outline-primary rounded-pill ms-2" :disabled="home" @click="emit('reset')">Now</button>
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
// The block-of-pages navigation bar used for weeks (sessions) and months
// (bookings). Drive it with usePagedWindow(); the slot holds extra controls.
defineProps<{
  indices: number[]
  offset: number
  home: boolean
  label: (index: number) => string
}>()
const emit = defineEmits<{ select: [index: number]; back: []; forward: []; reset: [] }>()
</script>
