<template>
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
          <td><RouterLink :to="{ name: 'attendance', query: { id: session.id, return: route.fullPath } }">{{ session.attended_count }} / {{ session.booking_count }}</RouterLink></td>
          <td>
            <RouterLink v-if="session.avg_rating != null" :to="{ name: 'feedback', query: { id: session.id, return: route.fullPath } }">{{ Math.round(session.avg_rating * 100) / 100 }}</RouterLink>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import {
  displayFullDate, displayVenueTime, endOfMonth, listSessionsFiltered, listUsers, startOfMonth,
  type Session, type UserSummary
} from '@pfnext/shared'
import PageTitle from '@/components/PageTitle.vue'
import { toDateInputValue, utcDayEnd, utcDayStart } from '@/composables/useDates'
import { useQueryState } from '@/composables/useQueryState'
import { user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const route = useRoute()
const query = useQueryState()

const filter = reactive({
  from: query.get('from') ?? toDateInputValue(startOfMonth(new Date())),
  to: query.get('to') ?? toDateInputValue(endOfMonth(new Date())),
  trainerId: query.get('trainer_id') ? Number(query.get('trainer_id')) : null
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
  query.update({
    from: filter.from || null,
    to: filter.to || null,
    trainer_id: filter.trainerId === null ? null : String(filter.trainerId)
  })
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
