<template>
  <RequireLogin>
    <div class="container">
      <PageTitle />
      <form v-if="draft" @submit.prevent="save">
        <table class="table">
          <tbody>
            <tr>
              <td>Email</td>
              <td>
                <div class="d-flex">
                  <div class="flex-grow-1 me-2">{{ draft.email }}</div>
                  <button type="submit" class="btn btn-sm btn-primary" :disabled="!dirty">Save</button>
                </div>
              </td>
            </tr>
            <tr>
              <td>Name</td>
              <td><input id="inputName" v-model="draft.name" class="form-control" type="text" required></td>
            </tr>
            <tr>
              <td>Phone</td>
              <td><input id="inputPhone" v-model="draft.phone" class="form-control" type="tel" required></td>
            </tr>
            <tr>
              <td>Emergency Contact</td>
              <td>
                <div class="row">
                  <div class="col-sm-6">
                    <input id="inputEmergencyName" v-model="draft.emergency_name" class="form-control" type="text" placeholder="Name" required>
                  </div>
                  <div class="col-sm-6">
                    <input id="inputEmergencyPhone" v-model="draft.emergency_phone" class="form-control" type="tel" placeholder="Phone" required>
                  </div>
                </div>
              </td>
            </tr>
            <tr>
              <td>Medical Information</td>
              <td>
                <textarea id="medicalText" v-model="draft.medical_info" class="form-control" rows="5" placeholder="Please state any relevant medical information, e.g. asthma, epilepsy, medications, allergies etc"></textarea>
              </td>
            </tr>
            <tr>
              <td>Credits</td>
              <td :class="draft.credits > 0 ? 'text-success' : 'text-secondary'">{{ draft.credits }}&nbsp;<i class="bi bi-ticket-perforated"></i></td>
            </tr>
            <tr>
              <td>Status</td>
              <td>
                <BibIcon :status="draft.status"><span class="text-secondary"><em>None</em></span></BibIcon>
                <span v-if="draft.status === 'green'">&nbsp;Green Bib Member</span>
                <span v-else-if="draft.status === 'red'">&nbsp;Red Bib Member</span>
                <span v-else-if="draft.status === 'blue'">&nbsp;Blue Bib Member</span>
              </td>
            </tr>
            <tr>
              <td>Roles</td>
              <td>
                <span v-for="role in draft.roles" :key="role">
                  <span class="badge rounded-pill text-bg-secondary">{{ role }}</span>&nbsp;
                </span>
              </td>
            </tr>
            <tr>
              <td>Password</td>
              <td>
                <a href="#" class="link-warning" @click.prevent="onResetPassword">Send a password reset email.</a>
                <br>
                <div v-if="resetMessage" class="my-1" :class="{ 'text-danger': resetMessage.isError }">{{ resetMessage.message }}</div>
              </td>
            </tr>
          </tbody>
        </table>
      </form>
    </div>
  </RequireLogin>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { getUser, patchUser, requestPasswordReset, type UserRecord } from '@pfnext/shared'
import BibIcon from '@/components/BibIcon.vue'
import PageTitle from '@/components/PageTitle.vue'
import RequireLogin from '@/components/RequireLogin.vue'
import { useDirtyTracking } from '@/composables/useDirtyTracking'
import { passwordResetUrl } from '@/composables/useUrlState'
import { user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const draft = ref<UserRecord | null>(null)
const { dirty, markClean } = useDirtyTracking(draft)
const resetMessage = ref<{ message: string; isError: boolean } | null>(null)

async function load() {
  if (!user.value) {
    return
  }
  const record = await tryApi(() => getUser(user.value!.id))
  if (record) {
    draft.value = record
    markClean()
  }
}

async function save() {
  if (!draft.value || !user.value) {
    return
  }
  const saved = await tryApi(async () => {
    await patchUser(user.value!.id, {
      name: draft.value!.name,
      phone: draft.value!.phone,
      emergency_name: draft.value!.emergency_name,
      emergency_phone: draft.value!.emergency_phone,
      medical_info: draft.value!.medical_info
    })
    return true
  })
  if (saved) {
    await load()
  }
}

async function onResetPassword() {
  if (!user.value) {
    return
  }
  resetMessage.value = { message: 'Processing...', isError: false }
  const email = user.value.email
  const message = await tryApi(() =>
    requestPasswordReset(email, document.location.host, passwordResetUrl())
  )
  if (message !== undefined) {
    resetMessage.value = { message, isError: false }
    window.location.href = '/passwordreset.html?email=' + encodeURIComponent(email)
  } else {
    resetMessage.value = null
  }
}

watch(user, load, { immediate: true })
</script>
