<template>
  <ion-page>
    <ion-header>
      <ion-toolbar>
        <ion-buttons slot="start">
          <ion-back-button default-href="/tabs/sessions" />
        </ion-buttons>
        <ion-title>{{ session?.session_type.name ?? 'Session' }}</ion-title>
      </ion-toolbar>
    </ion-header>
    <ion-content>
      <template v-if="session">
        <ion-list inset>
          <ion-item>
            <ion-label>
              <p>When</p>
              <h3>{{ displayLongDate(session.datetime) }}</h3>
              <h3>{{ displayTime(session.datetime) }} · {{ session.duration_mins }} mins</h3>
            </ion-label>
          </ion-item>
          <ion-item v-if="session.location">
            <ion-label>
              <p>Where</p>
              <h3>{{ session.location.name }}</h3>
              <p>{{ session.location.address }}</p>
            </ion-label>
          </ion-item>
          <ion-item v-if="session.trainer">
            <ion-label>
              <p>Trainer</p>
              <h3>{{ session.trainer.name }}</h3>
            </ion-label>
          </ion-item>
          <ion-item v-if="session.max_booking_count !== null">
            <ion-label>
              <p>Availability</p>
              <h3>{{ session.booking_count }} / {{ session.max_booking_count }} booked</h3>
            </ion-label>
            <session-status-badge :session="session" slot="end" />
          </ion-item>
          <ion-item v-if="session.cost > 0">
            <ion-label>
              <p>Cost</p>
              <h3>{{ session.cost }} credit{{ session.cost === 1 ? '' : 's' }} (Pay As You Go members)</h3>
            </ion-label>
          </ion-item>
          <ion-item v-if="session.notes">
            <ion-label class="ion-text-wrap">
              <p>Notes</p>
              <h3>{{ session.notes }}</h3>
            </ion-label>
          </ion-item>
        </ion-list>

        <div class="ion-padding">
          <template v-if="isPast(session.datetime)">
            <ion-text color="medium"><p>This session has already taken place.</p></ion-text>
          </template>
          <template v-else-if="session.booked">
            <ion-button expand="block" color="danger" fill="outline" @click="actions.cancel(session)">
              Cancel Booking
            </ion-button>
          </template>
          <template v-else-if="session.waitlist_rank !== null">
            <ion-text color="medium">
              <p>You are #{{ session.waitlist_rank }} on the waitlist. We’ll email you if a space opens up.</p>
            </ion-text>
            <ion-button expand="block" color="danger" fill="outline" @click="actions.leaveWaitlist(session)">
              Leave Waitlist
            </ion-button>
          </template>
          <template v-else-if="deadlinePassed">
            <ion-text color="medium"><p>The booking deadline for this session has passed.</p></ion-text>
          </template>
          <template v-else-if="full">
            <ion-text color="medium"><p>This session is fully booked.</p></ion-text>
            <ion-button expand="block" @click="actions.joinWaitlist(session)">Join Waitlist</ion-button>
          </template>
          <template v-else>
            <ion-button expand="block" @click="actions.book(session)">Book Session</ion-button>
          </template>
        </div>
      </template>
    </ion-content>
  </ion-page>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'
import {
  IonBackButton, IonButton, IonButtons, IonContent, IonHeader, IonItem,
  IonLabel, IonList, IonPage, IonText, IonTitle, IonToolbar,
  onIonViewWillEnter
} from '@ionic/vue'
import { getSession, isFull, isPastBookingDeadline } from '@/lib/bookingService'
import { useBookingActions } from '@/lib/useBookingActions'
import { displayLongDate, displayTime, isPast } from '@/lib/format'
import SessionStatusBadge from '@/components/SessionStatusBadge.vue'
import type { Session } from '@/types'

const route = useRoute()
const session = ref<Session | null>(null)

const full = computed(() => session.value !== null && isFull(session.value))
const deadlinePassed = computed(() => session.value !== null && isPastBookingDeadline(session.value))

async function load() {
  session.value = await getSession(Number(route.params.id))
}

const actions = useBookingActions(load)

onIonViewWillEnter(() => {
  void load()
})
</script>
