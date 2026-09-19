<template>
  <AuthCard title="Password Reset">
    <form @submit.prevent="onSubmit">
      <div class="mb-3">
        <label for="inputEmail" class="form-label">Email Address:</label>
        <input id="inputEmail" v-model="form.email" type="email" class="form-control">
      </div>
      <div class="mb-3">
        <label for="inputTempPassword" class="form-label">Temporary Password (from email):</label>
        <input id="inputTempPassword" v-model="form.tempPassword" class="form-control" autocomplete="off">
      </div>
      <div class="mb-3">
        <label for="inputNewPassword" class="form-label">New Password:</label>
        <input id="inputNewPassword" v-model="form.newPassword" type="password" class="form-control">
      </div>
      <div class="mb-3">
        <label for="inputNewPasswordRepeat" class="form-label">Repeat New Password:</label>
        <input id="inputNewPasswordRepeat" v-model="form.newPasswordRepeat" type="password" class="form-control">
      </div>
      <ul>
        <li v-for="error in validationErrors" :key="error" class="text-danger">{{ error }}</li>
      </ul>
      <button id="changePasswordSubmit" type="submit" class="btn btn-danger" :disabled="validationErrors.length > 0">Update Password</button>
      <div v-if="result" :class="{ 'text-danger': result.isError }">{{ result.message }}</div>
    </form>
  </AuthCard>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import { resetPassword } from '@pfnext/shared'
import AuthCard from '@/components/AuthCard.vue'
import { useQueryState } from '@/composables/useQueryState'
import { setUser } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const REDIRECT_TIMEOUT_MILLIS = 1000
const MIN_PASSWORD_LENGTH = 8

const router = useRouter()
const query = useQueryState()
const form = reactive({
  email: query.get('email') ?? '',
  tempPassword: query.get('temp_pwd') ?? '',
  newPassword: '',
  newPasswordRepeat: ''
})
const result = ref<{ message: string; isError: boolean } | null>(null)

const validationErrors = computed(() => {
  const errors: string[] = []
  if (form.newPassword === form.tempPassword) {
    errors.push('New password cannot be the same as the temporary password')
  }
  if (form.newPassword.length < MIN_PASSWORD_LENGTH) {
    errors.push(`Password must be at least ${MIN_PASSWORD_LENGTH} characters long`)
  }
  if (form.newPassword !== form.newPasswordRepeat) {
    errors.push('Passwords do not match')
  }
  return errors
})

async function onSubmit() {
  result.value = { message: 'Processing...', isError: false }
  const done = await tryApi(async () => {
    await resetPassword(form.email, form.tempPassword, form.newPassword, document.location.host)
    return true
  })
  if (done) {
    // Force a fresh login with the new password
    setUser(null)
    result.value = { message: 'Password successfully updated! Redirecting back to login...', isError: false }
    setTimeout(() => {
      void router.push({ name: 'login', query: { email: form.email } })
    }, REDIRECT_TIMEOUT_MILLIS)
  } else {
    result.value = null
  }
}
</script>
