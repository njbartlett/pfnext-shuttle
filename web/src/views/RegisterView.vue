<template>
  <AuthCard title="Registration">
    <form @submit.prevent="onSubmit">
      <div class="mb-3">
        <label for="inputName" class="form-label">Your Name:</label>
        <input id="inputName" v-model="form.name" type="text" class="form-control" :class="{ 'is-invalid': errors.name }" required>
        <div class="invalid-feedback">{{ errors.name }}</div>
      </div>

      <div class="row g-3 mb-3">
        <div class="col-md-6">
          <label for="inputEmail" class="form-label">Email Address:</label>
          <input id="inputEmail" v-model="form.email" type="email" class="form-control" :class="{ 'is-invalid': errors.email }" required>
          <div class="invalid-feedback">{{ errors.email }}</div>
        </div>
        <div class="col-md-6">
          <label for="inputPhone" class="form-label">Phone Number:</label>
          <input id="inputPhone" v-model="form.phone" type="tel" class="form-control" :class="{ 'is-invalid': errors.phone }" required>
          <div class="invalid-feedback">{{ errors.phone }}</div>
        </div>
      </div>

      <div class="row g-3 mb-3">
        <div class="col-md-6">
          <label for="emergencyContactName" class="form-label">Emergency Contact Name:</label>
          <input id="emergencyContactName" v-model="form.emergency_name" type="text" class="form-control" :class="{ 'is-invalid': errors.emergency_name }" required>
          <div class="invalid-feedback">{{ errors.emergency_name }}</div>
        </div>
        <div class="col-md-6">
          <label for="emergencyContactPhone" class="form-label">Emergency Contact Number:</label>
          <input id="emergencyContactPhone" v-model="form.emergency_phone" type="tel" class="form-control" :class="{ 'is-invalid': errors.emergency_phone }" required>
          <div class="invalid-feedback">{{ errors.emergency_phone }}</div>
        </div>
      </div>

      <div class="mb-3">
        <label for="medicalText" class="form-label">Medical Information:</label>
        <textarea id="medicalText" v-model="form.medical_info" class="form-control" rows="5" placeholder="Please state any relevant medical information, e.g. asthma, epilepsy, medications, allergies etc"></textarea>
      </div>

      <div class="mb-3">
        <label for="termsText" class="form-label">Terms and Conditions:</label>
        <textarea id="termsText" class="form-control" rows="10" readonly :value="TERMS"></textarea>
      </div>

      <div class="mb-3 form-check">
        <input id="termsAgreeCheck" v-model="form.terms_agreed" class="form-check-input" :class="{ 'is-invalid': errors.terms_agreed }" type="checkbox" required>
        <label class="form-check-label" for="termsAgreeCheck">I agree to the terms and conditions</label>
        <div class="invalid-feedback">{{ errors.terms_agreed }}</div>
      </div>

      <button id="submitButton" type="submit" class="btn btn-primary" :disabled="!ready">Register User</button>
      <div v-if="result">
        <div v-if="result.isExistingUser" class="text-danger">
          An account already exists with this email address. Please
          <a :href="'/login.html?email=' + encodeURIComponent(form.email)">login</a> instead.
        </div>
        <div v-else :class="{ 'text-danger': result.isError }">{{ result.message }}</div>
      </div>
    </form>
  </AuthCard>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import {
  ApiError, EMAIL_FORMAT_MESSAGE, PHONE_FORMAT_MESSAGE, isValidEmail, isValidPhone, registerUser
} from '@pfnext/shared'
import AuthCard from '@/components/AuthCard.vue'
import { passwordResetUrl } from '@/composables/useUrlState'

const HTTP_CONFLICT = 409

const TERMS =
  'By attending classes and using the park or venue or facilities and equipment, you hereby acknowledge and agree on behalf of yourself that you have voluntarily chosen to participate in intense physical exercise. We rely on you carrying out your own health self-assessment prior to taking part in any class. You agree to assume full responsibility for any and all injuries or damage to your person or property, which are sustained or aggravated by you in relation to the use of equipment and/or park or venue facilities.'

const form = reactive({
  name: '',
  email: '',
  phone: '',
  emergency_name: '',
  emergency_phone: '',
  medical_info: '',
  terms_agreed: false
})

const result = ref<{ message: string; isError: boolean; isExistingUser: boolean } | null>(null)

const errors = computed(() => ({
  name: form.name ? null : 'Name must not be empty',
  email: isValidEmail(form.email) ? null : EMAIL_FORMAT_MESSAGE,
  phone: isValidPhone(form.phone) ? null : PHONE_FORMAT_MESSAGE,
  emergency_name: form.emergency_name ? null : 'Emergency contact name must not be empty',
  emergency_phone: isValidPhone(form.emergency_phone) ? null : PHONE_FORMAT_MESSAGE,
  terms_agreed: form.terms_agreed ? null : 'You must agree to the terms and conditions before registering'
}))

const ready = computed(() => Object.values(errors.value).every((error) => error === null))

async function onSubmit() {
  result.value = { message: 'Processing...', isError: false, isExistingUser: false }
  try {
    await registerUser({
      name: form.name,
      email: form.email,
      phone: form.phone,
      emergency_name: form.emergency_name,
      emergency_phone: form.emergency_phone,
      medical_info: form.medical_info,
      website_url: document.location.host,
      reset_url: passwordResetUrl()
    })
    result.value = { message: 'Registration email sent!', isError: false, isExistingUser: false }
    window.location.href = '/passwordreset.html?email=' + encodeURIComponent(form.email)
  } catch (error) {
    result.value = {
      message: error instanceof Error ? error.message : String(error),
      isError: true,
      isExistingUser: error instanceof ApiError && error.status === HTTP_CONFLICT
    }
  }
}
</script>
