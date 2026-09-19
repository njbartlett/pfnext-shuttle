<template>
  <div class="card my-2">
    <div class="card-header d-flex align-items-center">
      <svg xmlns="http://www.w3.org/2000/svg" width="50" height="50" fill="currentColor" class="bi bi-sun-fill" viewBox="0 0 54 54">
        <path d="m 29.64,32.02 2.7,-4.68 8.16,14.04 h 5.4 C 44.7,39.46 33.3,19.96 32.34,18.04 L 20.58,38.44 16.2,46.06 H 0 C 0.9,44.56 6.96,34 12.78,23.86 16.2,17.92 19.98,11.26 24.24,4 h 5.4 L 18.48,23.32 8.1,41.38 h 5.4 C 20.88,28.36 25.5,20.68 32.34,8.68 32.76,9.4 53.88,46 54,46.06 H 37.8 Z"/>
      </svg>
      <h2 class="ms-2">Create Multiple Sessions</h2>
      <div class="ms-auto align-middle">
        <button type="button" class="btn-close" aria-label="Close" @click="goBack()"></button>
      </div>
    </div>
    <div class="card-body">
      <p class="text-secondary mb-3">
        Each row becomes one session. Duration is 60 minutes and the credit cost comes from the
        session type; use the <RouterLink to="/edit_session.html">single-session editor</RouterLink> for anything
        that needs different values.
      </p>

      <div class="bulk-grid-row bulk-grid-labels" aria-hidden="true">
        <div>Date</div><div>Time</div><div>Type</div><div>Trainer</div><div>Location</div>
        <div class="text-end">Cost</div><div></div>
      </div>

      <div v-for="(row, index) in rows" :key="row.key" class="bulk-grid-row">
        <input
          v-model="row.date"
          type="date"
          class="form-control bulk-field-label"
          data-label="Date"
          :class="{ 'is-invalid': rowStarted(row) && !row.date }"
          :aria-label="'Date, row ' + (index + 1)"
          required
        >
        <input
          v-model="row.time"
          type="time"
          class="form-control bulk-field-label"
          data-label="Time"
          list="bulkSessionTimeList"
          :class="{ 'is-invalid': rowStarted(row) && !row.time }"
          :aria-label="'Time, row ' + (index + 1)"
          required
        >
        <select
          v-model="row.session_type_id"
          class="form-control bulk-field-type"
          :class="{ 'is-invalid': rowStarted(row) && row.session_type_id === null }"
          :aria-label="'Session type, row ' + (index + 1)"
          required
        >
          <option disabled :value="null">Type&hellip;</option>
          <option v-for="sessionType in sessionTypes" :key="sessionType.id" :value="sessionType.id">{{ sessionType.name }}</option>
        </select>
        <select
          v-model="row.trainer_id"
          class="form-control bulk-field-trainer"
          :class="{ 'is-invalid': trainerError(row) !== null }"
          :aria-label="'Trainer, row ' + (index + 1)"
        >
          <option v-for="trainer in trainers" :key="trainer.id" :value="trainer.id">{{ trainer.name }}</option>
          <option :value="null">None</option>
        </select>
        <select v-model="row.location_id" class="form-control bulk-field-location" :aria-label="'Location, row ' + (index + 1)">
          <option v-for="location in locations" :key="location.id" :value="location.id">{{ location.name }}</option>
          <option :value="null">None/Other</option>
        </select>
        <div class="bulk-cost-cell" title="Defaulted from the session type">{{ costText(row) }}</div>
        <button
          type="button"
          class="btn btn-link bulk-btn-remove px-1"
          :class="rows.length === 1 ? 'text-secondary' : 'text-danger'"
          :disabled="rows.length === 1"
          :title="rows.length === 1 ? 'At least one row is required' : 'Remove row'"
          :aria-label="'Remove row ' + (index + 1)"
          @click="removeRow(index)"
        ><i class="bi bi-trash"></i></button>
        <div v-if="trainerError(row)" class="bulk-row-feedback">{{ trainerError(row) }}</div>
      </div>

      <datalist id="bulkSessionTimeList">
        <option v-for="time in SUGGESTED_TIMES" :key="time" :value="time"></option>
      </datalist>

      <div class="mt-2">
        <button id="bulkSessionAddRow" type="button" class="btn btn-outline-primary" @click="addRow"><i class="bi bi-plus-circle"></i>&nbsp;Add Row</button>
      </div>
    </div>

    <div class="d-flex card-footer align-items-center">
      <div class="me-auto p-2">
        <span v-if="outcome" :class="outcome.isError ? 'text-danger' : 'text-success'">{{ outcome.message }}</span>
      </div>
      <div class="p-2">
        <button id="bulkSessionSubmit" type="button" class="btn btn-primary" :disabled="!allValid" @click.prevent="createAll">
          Create {{ rows.length }} Session{{ rows.length === 1 ? '' : 's' }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// Bulk session entry: every row becomes one session. Duration is fixed at 60
// minutes and cost is defaulted from the session type; anything needing other
// values goes through edit_session.html instead.
import { computed, ref } from 'vue'
import {
  createSessionsBatch, listLocations, listSessionTypes, listUsers,
  type NewSession, type SessionLocation, type SessionType, type UserSummary
} from '@pfnext/shared'
import { useReturnPath } from '@/composables/useReturnPath'
import { tryApi } from '@/stores/apiError'

const DEFAULT_DURATION_MINS = 60
const SUGGESTED_TIMES = ['08:00', '08:30', '09:00', '09:30', '18:00', '18:30', '19:00', '19:30']

interface Row {
  key: number
  date: string | null
  time: string | null
  session_type_id: number | null
  trainer_id: number | null
  location_id: number | null
}

const { goBack } = useReturnPath('/sessions.html')

let nextRowKey = 1
function blankRow(): Row {
  return { key: nextRowKey++, date: null, time: null, session_type_id: null, trainer_id: null, location_id: null }
}

const rows = ref<Row[]>([blankRow()])
const outcome = ref<{ message: string; isError: boolean } | null>(null)
const sessionTypes = ref<SessionType[]>([])
const trainers = ref<UserSummary[]>([])
const locations = ref<SessionLocation[]>([])

function sessionTypeOf(row: Row): SessionType | null {
  return sessionTypes.value.find((t) => t.id === row.session_type_id) ?? null
}

function costText(row: Row): string {
  const sessionType = sessionTypeOf(row)
  return sessionType ? sessionType.cost + ' cr' : '—'
}

function trainerError(row: Row): string | null {
  const sessionType = sessionTypeOf(row)
  if (sessionType?.requires_trainer && row.trainer_id === null) {
    return `Sessions of type '${sessionType.name}' require a trainer to be selected.`
  }
  return null
}

// A row shows its missing-field highlighting only once the user has started it
function rowStarted(row: Row): boolean {
  return (
    row.date !== null || row.time !== null || row.session_type_id !== null || row.trainer_id !== null || row.location_id !== null
  )
}

function rowValid(row: Row): boolean {
  return !!row.date && !!row.time && row.session_type_id !== null && trainerError(row) === null
}

const allValid = computed(() => rows.value.length >= 1 && rows.value.every(rowValid))

function addRow() {
  // Copy the previous row so only what changes needs editing
  const previous = rows.value[rows.value.length - 1]
  rows.value.push({ ...previous, key: nextRowKey++ })
  outcome.value = null
}

function removeRow(index: number) {
  if (rows.value.length > 1) {
    rows.value.splice(index, 1)
  }
  outcome.value = null
}

async function createAll() {
  const request: NewSession[] = rows.value.map((row) => ({
    datetime: new Date(row.date + ' ' + row.time).toISOString(),
    duration_mins: DEFAULT_DURATION_MINS,
    session_type_id: row.session_type_id ?? 0,
    location_id: row.location_id,
    trainer_id: row.trainer_id,
    cost: sessionTypeOf(row)?.cost ?? 0,
    max_bookings: null,
    notes: null,
    booking_deadline_mins: 0
  }))
  const ids = await tryApi(() => createSessionsBatch(request))
  if (ids) {
    outcome.value = { message: `Created ${ids.length} session${ids.length === 1 ? '' : 's'}!`, isError: false }
    rows.value = [blankRow()]
  }
}

void Promise.all([
  tryApi(() => listSessionTypes(false)),
  tryApi(() => listUsers({ role: 'trainer' })),
  tryApi(() => listLocations())
]).then(([types, trainerList, locationList]) => {
  sessionTypes.value = types ?? []
  trainers.value = trainerList ?? []
  locations.value = locationList ?? []
})
</script>

<style>
/* One shared column template for the label row and every session row */
.bulk-grid-row {
    display: grid;
    grid-template-columns: 10.5rem 7rem 1.2fr 1fr 1fr 3.5rem 2.5rem;
    gap: 8px;
    align-items: start;
    margin-bottom: 8px;
}
.bulk-grid-labels {
    margin-bottom: 4px;
    color: var(--bs-secondary-color);
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
}
.bulk-cost-cell {
    align-self: center;
    text-align: right;
    padding-right: 4px;
    color: var(--bs-secondary-color);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
}
.bulk-row-feedback {
    grid-column: 1 / -1;
    margin: -2px 0 2px;
    color: var(--bs-form-invalid-color);
    font-size: 0.85rem;
}
/* Phone width: each session becomes its own bordered block, fields wrap two-up */
@media (max-width: 760px) {
    .bulk-grid-labels { display: none; }
    .bulk-grid-row {
        grid-template-columns: 1fr 1fr;
        border: 1px solid var(--bs-border-color);
        border-radius: var(--bs-border-radius);
        padding: 10px;
        position: relative;
    }
    .bulk-grid-row > .bulk-cost-cell { text-align: left; align-self: center; }
    .bulk-grid-row > .bulk-btn-remove { position: absolute; top: 6px; right: 6px; }
    .bulk-field-type, .bulk-field-trainer, .bulk-field-location { grid-column: 1 / -1; }
    .bulk-field-label::before {
        content: attr(data-label);
        display: block;
        color: var(--bs-secondary-color);
        font-size: 0.7rem;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.06em;
        margin-bottom: 3px;
    }
}
</style>
