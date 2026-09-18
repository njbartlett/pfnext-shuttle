<template>
  <ion-page>
    <ion-header>
      <ion-toolbar>
        <ion-title>Profile</ion-title>
      </ion-toolbar>
    </ion-header>
    <ion-content>
      <ion-list inset v-if="user">
        <ion-item>
          <ion-label>
            <p>Name</p>
            <h3>{{ user.name }}</h3>
          </ion-label>
        </ion-item>
        <ion-item>
          <ion-label>
            <p>Email</p>
            <h3>{{ user.email }}</h3>
          </ion-label>
        </ion-item>
        <ion-item>
          <ion-label>
            <p>Roles</p>
            <h3>{{ rolesText }}</h3>
          </ion-label>
        </ion-item>
        <ion-item v-if="record">
          <ion-label>
            <p>Pay As You Go credits</p>
            <h3>{{ record.credits }}</h3>
          </ion-label>
        </ion-item>
      </ion-list>

      <div class="ion-padding">
        <ion-button expand="block" color="danger" fill="outline" @click="onLogout">Log Out</ion-button>
      </div>
    </ion-content>
  </ion-page>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import {
  IonButton, IonContent, IonHeader, IonItem, IonLabel, IonList, IonPage,
  IonTitle, IonToolbar, onIonViewWillEnter
} from '@ionic/vue'
import { logout, user } from '@/lib/auth'
import { getMyUserRecord } from '@/lib/bookingService'
import type { UserRecord } from '@pfnext/shared'

const router = useRouter()
const record = ref<UserRecord | null>(null)

const rolesText = computed(() => {
  const roles = user.value?.roles ?? []
  return roles.length > 0 ? roles.join(', ') : 'None'
})

onIonViewWillEnter(() => {
  void getMyUserRecord().then((r) => {
    record.value = r
  }).catch(() => {
    // Credits are nice-to-have here; the page still renders without them
  })
})

async function onLogout() {
  await logout()
  await router.replace('/login')
}
</script>
