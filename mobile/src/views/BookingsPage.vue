<template>
  <ion-page>
    <ion-header>
      <ion-toolbar>
        <ion-title>My Bookings</ion-title>
      </ion-toolbar>
    </ion-header>
    <ion-content>
      <ion-refresher slot="fixed" @ionRefresh="onRefresh">
        <ion-refresher-content />
      </ion-refresher>

      <ion-list v-if="bookings.length > 0">
        <ion-item-sliding v-for="booking in bookings" :key="booking.session_id">
          <ion-item button :router-link="`/tabs/sessions/${booking.session_id}`" :detail="true">
            <ion-label>
              <h2>{{ booking.session_type.name }}</h2>
              <p class="session-meta">
                {{ displayDate(booking.session_datetime) }} · {{ displayTime(booking.session_datetime) }}
                <template v-if="booking.session_location"> · {{ booking.session_location.name }}</template>
              </p>
            </ion-label>
            <ion-badge v-if="booking.credits_used > 0" color="medium" slot="end">
              {{ booking.credits_used }} cr
            </ion-badge>
          </ion-item>
          <ion-item-options side="end">
            <ion-item-option color="danger" @click="onCancel(booking)">Cancel</ion-item-option>
          </ion-item-options>
        </ion-item-sliding>
      </ion-list>

      <div v-else-if="loaded" class="ion-padding ion-text-center">
        <p>No upcoming bookings.</p>
        <ion-button router-link="/tabs/sessions" fill="outline">Browse Sessions</ion-button>
      </div>
    </ion-content>
  </ion-page>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import {
  IonBadge, IonButton, IonContent, IonHeader, IonItem, IonItemOption,
  IonItemOptions, IonItemSliding, IonLabel, IonList, IonPage, IonRefresher,
  IonRefresherContent, IonTitle, IonToolbar,
  alertController, onIonViewWillEnter, toastController
} from '@ionic/vue'
import { cancelBooking, listMyBookings } from '@/lib/bookingService'
import { displayDate, displayTime } from '@pfnext/shared'
import type { Booking } from '@pfnext/shared'

const DAYS_AHEAD = 60

const bookings = ref<Booking[]>([])
const loaded = ref(false)

async function load() {
  const from = new Date()
  const to = new Date(from.getTime() + DAYS_AHEAD * 24 * 60 * 60 * 1000)
  bookings.value = await listMyBookings(from, to)
  loaded.value = true
}

async function onCancel(booking: Booking) {
  const alert = await alertController.create({
    header: 'Cancel booking?',
    message: 'Any credits spent on this booking will be refunded.',
    buttons: [
      { text: 'Keep booking', role: 'cancel' },
      {
        text: 'Cancel booking',
        role: 'destructive',
        handler: () => {
          void (async () => {
            try {
              await cancelBooking(booking.session_id)
              const toast = await toastController.create({
                message: 'Booking cancelled',
                duration: 2500,
                color: 'success'
              })
              await toast.present()
              await load()
            } catch (e) {
              const toast = await toastController.create({
                message: e instanceof Error ? e.message : 'Failed to cancel booking',
                duration: 3000,
                color: 'danger'
              })
              await toast.present()
            }
          })()
        }
      }
    ]
  })
  await alert.present()
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
