<template>
  <ion-page>
    <ion-content class="ion-padding">
      <div class="login-wrap">
        <h1>Another Level</h1>
        <p>Sign in with your membership account to book sessions.</p>

        <form @submit.prevent="onSubmit">
          <ion-list inset>
            <ion-item>
              <ion-input
                v-model="email"
                label="Email"
                label-placement="floating"
                type="email"
                autocomplete="email"
                inputmode="email"
                required
              />
            </ion-item>
            <ion-item>
              <ion-input
                v-model="password"
                label="Password"
                label-placement="floating"
                type="password"
                autocomplete="current-password"
                required
              />
            </ion-item>
          </ion-list>

          <ion-text v-if="error" color="danger">
            <p class="ion-padding-start">{{ error }}</p>
          </ion-text>

          <ion-button type="submit" expand="block" :disabled="busy" class="ion-margin">
            <ion-spinner v-if="busy" name="crescent" />
            <span v-else>Log In</span>
          </ion-button>
        </form>
      </div>
    </ion-content>
  </ion-page>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { IonButton, IonContent, IonInput, IonItem, IonList, IonPage, IonSpinner, IonText } from '@ionic/vue'
import { login } from '@/lib/auth'

const router = useRouter()
const route = useRoute()

const email = ref('')
const password = ref('')
const error = ref<string | null>(null)
const busy = ref(false)

async function onSubmit() {
  error.value = null
  busy.value = true
  try {
    await login(email.value.trim(), password.value)
    const returnPath = typeof route.query.return === 'string' ? route.query.return : '/tabs/sessions'
    await router.replace(returnPath)
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Login failed'
  } finally {
    busy.value = false
  }
}
</script>

<style scoped>
.login-wrap {
  max-width: 26rem;
  margin: 15vh auto 0;
}
.login-wrap h1 {
  text-align: center;
}
.login-wrap p {
  text-align: center;
  color: var(--ion-color-medium);
}
</style>
