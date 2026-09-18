<template>
  <ion-badge v-if="session.booked" color="success">Booked</ion-badge>
  <ion-badge v-else-if="session.waitlist_rank !== null" color="warning">
    Waitlist #{{ session.waitlist_rank }}
  </ion-badge>
</template>

<script setup lang="ts">
import { IonBadge } from '@ionic/vue'
import type { Session } from '@pfnext/shared'

defineProps<{ session: Session }>()
</script>
