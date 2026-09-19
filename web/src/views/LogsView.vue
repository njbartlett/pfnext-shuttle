<template>
  <div class="container">
    <PageTitle />

    <div class="row">
      <div class="col">
        <div class="card my-2">
          <h6 class="card-header">Filter</h6>
          <div class="card-body">
            <form @submit.prevent>
              <label for="fromDate" class="form-label">Date</label>
              <div class="input-group">
                <button type="button" class="btn btn-primary" @click="date = shiftDateInput(date, -1)">&laquo; Prev</button>
                <input id="fromDate" v-model="date" class="form-control" type="date">
                <button type="button" class="btn btn-primary" @click="date = shiftDateInput(date, 1)">Next &raquo;</button>
              </div>
            </form>
          </div>
        </div>
      </div>
    </div>

    <table class="table align-middle">
      <thead>
        <tr>
          <th scope="col">Date &amp; Time</th>
          <th scope="col">User</th>
          <th scope="col">Type</th>
          <th scope="col">Detail</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="entry in entries" :key="entry.id">
          <td>{{ displayDateTime(entry.datetime) }}</td>
          <td>{{ entry.person }}</td>
          <td>{{ entry.event_type }}</td>
          <td>{{ entry.detail }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { displayDateTime, readLog, type LogRow } from '@pfnext/shared'
import PageTitle from '@/components/PageTitle.vue'
import { shiftDateInput, toDateInputValue, utcDayEnd, utcDayStart } from '@/composables/useDates'
import { user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const date = ref(toDateInputValue(new Date()))
const entries = ref<LogRow[]>([])

async function load() {
  if (!user.value || !date.value) {
    entries.value = []
    return
  }
  entries.value = (await tryApi(() => readLog(utcDayStart(date.value), utcDayEnd(date.value)))) ?? []
}

watch([date, user], load, { immediate: true })
</script>
