<template>
  <span
    v-if="session.cost === 0"
    class="badge rounded-pill text-bg-success"
    title="Free session: no credits used, and it doesn't count towards a weekly session limit"
  >
    Free
  </span>
</template>

<script setup lang="ts">
// "Free" pill for zero-cost sessions, which never spend credits and never
// use up a limited member's one session a week (see check_bookable in
// src/bookings.rs)
import type { Session } from '@pfnext/shared'

defineProps<{ session: Session }>()
</script>
