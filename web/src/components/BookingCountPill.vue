<template>
  <span
    v-if="session.max_booking_count"
    class="badge rounded-pill"
    :class="isFull(session) ? 'text-bg-danger' : 'text-bg-primary'"
    :title="session.booking_count + ' member(s) booked of maximum ' + session.max_booking_count"
  >
    {{ session.booking_count }}/{{ session.max_booking_count }}
  </span>
  <span v-else class="badge rounded-pill text-bg-primary" :title="session.booking_count + ' member(s) booked'">
    {{ session.booking_count }}
  </span>
</template>

<script setup lang="ts">
// "booked / capacity" pill, red when the session is full
import { isFull, type Session } from '@pfnext/shared'

defineProps<{ session: Session }>()
</script>
