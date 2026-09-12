<template>
  <ion-page>
    <ion-header>
      <ion-toolbar>
        <ion-title>Sessions</ion-title>
      </ion-toolbar>
    </ion-header>
    <ion-content>
      <ion-refresher slot="fixed" @ionRefresh="onRefresh">
        <ion-refresher-content />
      </ion-refresher>

      <ion-list v-if="days.length > 0">
        <template v-for="group in days" :key="group.day">
          <ion-item-divider sticky>
            <ion-label>{{ displayDate(group.items[0].datetime) }}</ion-label>
          </ion-item-divider>
          <ion-item
            v-for="session in group.items"
            :key="session.id"
            button
            :router-link="`/tabs/sessions/${session.id}`"
            :detail="true"
          >
            <ion-label>
              <h2>{{ session.session_type.name }}</h2>
              <p class="session-meta">
                {{ displayTime(session.datetime) }} · {{ session.duration_mins }} mins
                <template v-if="session.location"> · {{ session.location.name }}</template>
              </p>
              <p v-if="spacesText(session)" class="session-meta">{{ spacesText(session) }}</p>
            </ion-label>
            <session-status-badge :session="session" slot="end" />
          </ion-item>
        </template>
      </ion-list>

      <div v-else-if="loaded" class="ion-padding ion-text-center">
        <p>No upcoming sessions in the next two weeks.</p>
      </div>
    </ion-content>
  </ion-page>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  IonContent, IonHeader, IonItem, IonItemDivider, IonLabel, IonList,
  IonPage, IonRefresher, IonRefresherContent, IonTitle, IonToolbar,
  onIonViewWillEnter
} from '@ionic/vue'
import { listSessions, spacesLeft } from '@/lib/bookingService'
import { displayDate, displayTime, groupByDay } from '@/lib/format'
import SessionStatusBadge from '@/components/SessionStatusBadge.vue'
import type { Session } from '@/types'

const DAYS_AHEAD = 14

const sessions = ref<Session[]>([])
const loaded = ref(false)

const days = computed(() => groupByDay(sessions.value, (s) => s.datetime))

async function load() {
  const from = new Date()
  const to = new Date(from.getTime() + DAYS_AHEAD * 24 * 60 * 60 * 1000)
  sessions.value = await listSessions(from, to)
  loaded.value = true
}

function spacesText(session: Session): string | null {
  const spaces = spacesLeft(session)
  if (spaces === null) {
    return null
  }
  return spaces === 0 ? 'Fully booked' : `${spaces} space${spaces === 1 ? '' : 's'} left`
}

onIonViewWillEnter(() => {
  void load()
})

async function onRefresh(event: CustomEvent) {
  try {
    await load()
  } finally {
    ;(event.target as HTMLIonRefresherElement).complete()
  }
}
</script>
