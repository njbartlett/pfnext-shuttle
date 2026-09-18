<template>
  <RequireLogin>
    <div class="container">
      <PageTitle />

      <form @submit.prevent>
        <div class="row">
          <div class="col-2"><label for="rangeFrom" class="col-form-label">From:</label></div>
          <div class="col-4"><input id="rangeFrom" v-model="filter.from" type="date" class="form-control" aria-label="start date"></div>
          <div class="col-2"><label for="rangeTo" class="col-form-label">To:</label></div>
          <div class="col-4"><input id="rangeTo" v-model="filter.to" type="date" class="form-control" aria-label="end date"></div>
        </div>
        <div class="row mt-1">
          <div class="col-2"><label for="filterTrainer" class="col-form-label">Trainer:</label></div>
          <div class="col-4">
            <select id="filterTrainer" v-model="filter.trainerId" class="form-control">
              <option :value="null">Any</option>
              <option v-for="trainer in trainers" :key="trainer.id" :value="trainer.id">{{ trainer.name }}</option>
            </select>
          </div>
        </div>
      </form>

      <table class="table table-striped my-3">
        <thead>
          <tr>
            <th scope="col">Date/Time</th>
            <th scope="col">Type</th>
            <th scope="col">Location</th>
            <th scope="col">Trainer</th>
            <th scope="col">Attendance</th>
            <th scope="col">Avg.Rating</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="session in sessions" :key="session.id">
            <td>{{ displayFullDate(session.datetime) }} {{ displayVenueTime(session.datetime) }}</td>
            <td>{{ session.session_type.name }}</td>
            <td>{{ session.location ? session.location.name : 'N/A' }}</td>
            <td>{{ session.trainer ? session.trainer.name : 'N/A' }}</td>
            <td><a :href="'/attendance.html?id=' + session.id + '&return=' + loginReturnUrl()">{{ session.attended_count }} / {{ session.booking_count }}</a></td>
            <td>
              <a v-if="session.avg_rating != null" :href="'/feedback.html?id=' + session.id + '&return=' + loginReturnUrl()">{{ Math.round(session.avg_rating * 100) / 100 }}</a>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </RequireLogin>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import {
  displayFullDate, displayVenueTime, endOfMonth, listSessionsFiltered, listUsers, startOfMonth,
  type Session, type UserSummary
} from '@pfnext/shared'
import PageTitle from '@/components/PageTitle.vue'
import RequireLogin from '@/components/RequireLogin.vue'
import { toDateInputValue, utcDayEnd, utcDayStart } from '@/composables/useDates'
import { useHashState } from '@/composables/useUrlState'
import { loginReturnUrl, user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const hash = useHashState()

const filter = reactive({
  from: hash.get('from') ?? toDateInputValue(startOfMonth(new Date())),
  to: hash.get('to') ?? toDateInputValue(endOfMonth(new Date())),
  trainerId: hash.get('trainer_id') ? Number(hash.get('trainer_id')) : null
})
const trainers = ref<UserSummary[]>([])
const sessions = ref<Session[]>([])

async function load() {
  if (!user.value) {
    sessions.value = []
    return
  }
  sessions.value =
    (await tryApi(() =>
      listSessionsFiltered({
        attended: true,
        from: filter.from ? utcDayStart(filter.from) : undefined,
        to: filter.to ? utcDayEnd(filter.to) : undefined,
        trainerId: filter.trainerId ?? undefined
      })
    )) ?? []
}

watch(filter, () => {
  hash.set('from', filter.from || null)
  hash.set('to', filter.to || null)
  hash.set('trainer_id', filter.trainerId === null ? null : String(filter.trainerId))
  void load()
})

watch(
  user,
  async (current) => {
    await load()
    trainers.value = current ? ((await tryApi(() => listUsers({ role: 'trainer' }))) ?? []) : []
  },
  { immediate: true }
)
</script>
