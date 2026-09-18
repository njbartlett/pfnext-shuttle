<template>
  <RequireLogin>
    <div class="container">
      <PageTitle />
      <table class="table table-sm align-center">
        <thead>
          <tr>
            <th>User</th>
            <th>Logged In</th>
            <th>Expiry</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="login in logins" :key="login.sessionid">
            <td class="text-secondary">
              {{ login.name }} <span class="text-primary">&lt;{{ login.email }}&gt;</span>
              <span v-if="login.is_current" class="text-success" title="Current Login">&nbsp;<i class="bi bi-person-circle"></i></span>
            </td>
            <td class="text-secondary">
              <span>{{ displayDateTime(login.loggedin) }}</span>
              <button class="btn btn-sm text-toggle me-1" type="button" data-bs-toggle="collapse" :data-bs-target="'#loggedinCollapse' + login.sessionid" aria-expanded="false" :aria-controls="'loggedinCollapse' + login.sessionid">
                <span><i class="bi bi-info-circle"></i></span>
              </button>
              <div :id="'loggedinCollapse' + login.sessionid" class="collapse">
                <div class="card card-body mt-1">{{ login.loggedin_from }}</div>
              </div>
            </td>
            <td class="text-secondary">
              {{ displayDateTime(login.expiry) }}
              <button v-if="!login.is_current" class="btn btn-sm btn-outline-danger float-end" title="Expire Now" @click="expire(login)"><i class="bi bi-trash"></i></button>
            </td>
          </tr>
        </tbody>
      </table>

      <p class="text-secondary">All times shown for timezone <span class="fst-italic">{{ TIMEZONE }}</span>.</p>
    </div>
  </RequireLogin>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { TIMEZONE, deleteLoginSession, displayDateTime, listLoginSessions, type LoginSessionRecord } from '@pfnext/shared'
import PageTitle from '@/components/PageTitle.vue'
import RequireLogin from '@/components/RequireLogin.vue'
import { user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const logins = ref<LoginSessionRecord[]>([])

async function load() {
  logins.value = user.value ? ((await tryApi(listLoginSessions)) ?? []) : []
}

async function expire(login: LoginSessionRecord) {
  const done = await tryApi(async () => {
    await deleteLoginSession(login.sessionid)
    return true
  })
  if (done) {
    await load()
  }
}

watch(user, load, { immediate: true })
</script>
