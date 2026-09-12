<template>
  <ion-page>
    <ion-tabs>
      <ion-router-outlet />
      <ion-tab-bar slot="bottom">
        <ion-tab-button tab="sessions" href="/tabs/sessions">
          <ion-icon :icon="calendarOutline" />
          <ion-label>Sessions</ion-label>
        </ion-tab-button>
        <ion-tab-button tab="bookings" href="/tabs/bookings">
          <ion-icon :icon="checkmarkCircleOutline" />
          <ion-label>My Bookings</ion-label>
        </ion-tab-button>
        <ion-tab-button tab="profile" href="/tabs/profile">
          <ion-icon :icon="personOutline" />
          <ion-label>Profile</ion-label>
        </ion-tab-button>
      </ion-tab-bar>
    </ion-tabs>
  </ion-page>
</template>

<script setup lang="ts">
import { IonIcon, IonLabel, IonPage, IonRouterOutlet, IonTabBar, IonTabButton, IonTabs } from '@ionic/vue'
import { calendarOutline, checkmarkCircleOutline, personOutline } from 'ionicons/icons'
</script>
