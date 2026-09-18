<template>
  <!-- Already logged in -->
  <div v-if="user" class="alert alert-success my-5 text-center" role="alert">
    <i class="bi bi-info-circle"></i>&nbsp;You are logged in as {{ user.name }}. Open the
    <a href="/sessions.html" class="alert-link">sessions page</a> or
    <a href="#" class="alert-link" @click.prevent="logout({ stay: true })">login as a different user</a>.
  </div>

  <template v-else>
    <AuthCard title="Login">
      <form @submit.prevent="onLogin">
        <div class="mb-3">
          <label for="inputEmail" class="form-label">Email Address:</label>
          <input id="inputEmail" v-model="form.email" type="email" class="form-control" :class="{ 'is-invalid': emailError }" required>
          <div class="invalid-feedback">{{ emailError }}</div>
        </div>

        <div v-if="!showForgotten">
          <div class="mb-3">
            <label for="inputPassword" class="form-label">Password</label>
            <input id="inputPassword" v-model="form.password" type="password" class="form-control">
          </div>
          <button id="loginSubmit" type="submit" class="btn btn-primary" :disabled="!ready">Login</button>
          <a href="#" @click.prevent="showForgotten = true">Forgotten&nbsp;my&nbsp;password</a>
        </div>
      </form>

      <div v-if="showForgotten">
        <p class="card-text">If you have forgotten your password, enter your email address above and click &ldquo;Reset&rdquo;. If an account exists with that address, then an email will be sent containing reset instructions.</p>
        <button id="forgottenPasswordSubmit" type="button" class="btn btn-danger" :disabled="!ready" @click.prevent="onResetPassword">Reset</button>
        <button type="button" class="btn btn-outline-secondary" @click.prevent="showForgotten = false">Cancel</button>
        <div v-if="resetResult" :class="{ 'text-danger': resetResult.isError }">{{ resetResult.message }}</div>
      </div>
    </AuthCard>

    <div class="text-center my-3">
      <p class="fs-5">No account yet? <a href="/register.html">Sign Up!</a></p>
    </div>
  </template>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { EMAIL_FORMAT_MESSAGE, isValidEmail, requestPasswordReset } from '@pfnext/shared'
import AuthCard from '@/components/AuthCard.vue'
import { useReturnPath } from '@/composables/useReturnPath'
import { passwordResetUrl, urlQuery } from '@/composables/useUrlState'
import { login, logout, user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const { returnPath } = useReturnPath('/index.html')

const form = reactive({
  email: urlQuery().get('email') ?? '',
  password: ''
})
const showForgotten = ref(false)
const resetResult = ref<{ message: string; isError: boolean } | null>(null)

const emailError = computed(() => (isValidEmail(form.email) ? null : EMAIL_FORMAT_MESSAGE))
const ready = computed(() => emailError.value === null)

async function onLogin() {
  const loggedIn = await tryApi(() => login(form.email, form.password))
  if (loggedIn) {
    window.location.href = returnPath ?? '/index.html'
  }
}

async function onResetPassword() {
  resetResult.value = { message: 'Processing...', isError: false }
  const message = await tryApi(() =>
    requestPasswordReset(form.email, document.location.host, passwordResetUrl())
  )
  if (message !== undefined) {
    resetResult.value = { message, isError: false }
    window.location.href = '/passwordreset.html?email=' + encodeURIComponent(form.email)
  } else {
    resetResult.value = null
  }
}
</script>
