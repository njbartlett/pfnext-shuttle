<template>
  <div class="card my-2">
    <div class="card-header d-flex align-items-center">
      <svg xmlns="http://www.w3.org/2000/svg" width="50" height="50" fill="currentColor" class="bi bi-sun-fill" viewBox="0 0 54 54">
        <path d="m 29.64,32.02 2.7,-4.68 8.16,14.04 h 5.4 C 44.7,39.46 33.3,19.96 32.34,18.04 L 20.58,38.44 16.2,46.06 H 0 C 0.9,44.56 6.96,34 12.78,23.86 16.2,17.92 19.98,11.26 24.24,4 h 5.4 L 18.48,23.32 8.1,41.38 h 5.4 C 20.88,28.36 25.5,20.68 32.34,8.68 32.76,9.4 53.88,46 54,46.06 H 37.8 Z"/>
      </svg>
      <h2 class="ms-2">{{ form.id ? 'Edit' : 'Create New' }} Session</h2>
      <div class="ms-auto align-middle">
        <button type="button" class="btn-close" aria-label="Close" @click="goBack()"></button>
      </div>
    </div>
    <div class="card-body">
      <form @submit.prevent="save(false)">
        <!-- Date, time and duration -->
        <div class="mb-2 row">
          <div class="col-4">
            <label for="editSessionDate" class="form-label">Date</label>
            <input id="editSessionDate" v-model="form.date" class="form-control" :class="{ 'is-invalid': errors.date }" type="date" required>
            <div class="invalid-feedback">{{ errors.date }}</div>
          </div>
          <div class="col-4">
            <label for="editSessionTime" class="form-label">Time</label>
            <input id="editSessionTime" v-model="form.time" class="form-control" :class="{ 'is-invalid': errors.time }" type="time" list="editSessionTimeList" required>
            <datalist id="editSessionTimeList">
              <option v-for="time in SUGGESTED_TIMES" :key="time" :value="time"></option>
            </datalist>
            <div class="invalid-feedback">{{ errors.time }}</div>
          </div>
          <div class="col-4">
            <label for="editSessionDuration" class="form-label">Duration</label>
            <div class="input-group">
              <input id="editSessionDuration" v-model.number="form.duration_mins" class="form-control no-spinner" :class="{ 'is-invalid': errors.duration_mins }" type="number" min="0" step="5" required>
              <span class="input-group-text">mins</span>
              <div class="invalid-feedback">{{ errors.duration_mins }}</div>
            </div>
          </div>
        </div>

        <!-- Type, cost and limit -->
        <div class="mb-2 row">
          <div class="col-4">
            <label for="editSessionType" class="form-label">Type</label>
            <select id="editSessionType" v-model="form.session_type_id" class="form-control" :class="{ 'is-invalid': errors.session_type_id }" required @change="onChangeSessionType">
              <option disabled :value="null">Type&hellip;</option>
              <option v-for="sessionType in sessionTypes" :key="sessionType.id" :value="sessionType.id">{{ sessionType.name }}</option>
            </select>
            <div class="invalid-feedback">{{ errors.session_type_id }}</div>
          </div>
          <div class="col-4">
            <label for="editSessionCost" class="form-label">Cost</label>
            <input id="editSessionCost" v-model.number="form.cost" type="number" class="form-control" :class="{ 'is-invalid': errors.cost }" title="Credits cost" aria-label="Credits cost">
            <div class="invalid-feedback">{{ errors.cost }}</div>
          </div>
          <div class="col-4">
            <label for="editSessionBookingLimit" class="form-label">Max. Bookings</label>
            <div class="input-group">
              <div class="input-group-text">
                <input v-model="form.max_bookings_enabled" class="form-check-input mt-0" type="checkbox" aria-label="Enable max booking limit" title="Enable max bookings limit">
              </div>
              <input id="editSessionBookingLimit" v-model.number="form.max_bookings" type="number" class="form-control" :class="{ 'is-invalid': errors.max_bookings }" :disabled="!form.max_bookings_enabled" title="Max bookings limit" aria-label="Max bookings limit">
              <div class="invalid-feedback">{{ errors.max_bookings }}</div>
            </div>
          </div>
        </div>

        <!-- Trainer, location and deadline -->
        <div class="mb-2 row">
          <div class="col">
            <label for="editSessionTrainer" class="form-label">Trainer</label>
            <select id="editSessionTrainer" v-model="form.trainer_id" class="form-control" :class="{ 'is-invalid': errors.trainer_id }" required>
              <option v-for="trainer in trainers" :key="trainer.id" :value="trainer.id">{{ trainer.name }}</option>
              <option :value="null">None</option>
            </select>
            <div class="invalid-feedback">{{ errors.trainer_id }}</div>
          </div>
          <div class="col">
            <label for="editSessionLocation" class="form-label">Location</label>
            <select id="editSessionLocation" v-model="form.location_id" class="form-control" required>
              <option v-for="location in locations" :key="location.id" :value="location.id">{{ location.name }}</option>
              <option :value="null">None/Other</option>
            </select>
          </div>
          <div class="col">
            <label for="editSessionBookingDeadlineHours" class="form-label">Booking Deadline</label>
            <div class="input-group flex-nowrap">
              <input id="editSessionBookingDeadlineHours" v-model.number="form.booking_deadline_hours" class="form-control no-spinner" type="number" min="0" required>
              <span class="input-group-text">h</span>
              <input id="editSessionBookingDeadlineMins" v-model.number="form.booking_deadline_mins" class="form-control no-spinner" type="number" min="0" max="59" step="5" required>
              <span class="input-group-text">m</span>
            </div>
            <div v-if="deadlineText" class="text-secondary"><i class="bi bi-info-circle"></i> Deadline to book is {{ deadlineText }}</div>
          </div>
        </div>

        <div class="mb-2">
          <label for="editSessionNotes" class="form-label">Notes</label>
          <textarea id="editSessionNotes" v-model="form.notes" class="form-control" rows="5"></textarea>
        </div>
      </form>
    </div>

    <div class="d-flex card-footer align-items-center">
      <div class="me-auto p-2">
        <span v-if="outcome" :class="outcome.isError ? 'text-danger' : 'text-success'">{{ outcome.message }}</span>
      </div>
      <div class="p-2">
        <button id="editSessionSubmit" type="button" class="btn btn-primary" :disabled="!ready" @click.prevent="save(false)">Save</button>
      </div>
      <div class="p-2">
        <button id="editSessionSubmitAndCreate" type="button" class="btn btn-primary" :disabled="!ready" @click.prevent="save(true)">Save &amp; Create Another</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import {
  createSession, getSession, listLocations, listSessionTypes, listUsers, updateSession,
  type NewSession, type Session, type SessionLocation, type SessionType, type UserSummary
} from '@pfnext/shared'
import { useQueryState } from '@/composables/useQueryState'
import { useReturnPath } from '@/composables/useReturnPath'
import { tryApi } from '@/stores/apiError'

const SUGGESTED_TIMES = ['08:00', '08:30', '09:00', '09:30', '18:00', '18:30', '19:00', '19:30']

const { goBack } = useReturnPath('/sessions.html')
const query = useQueryState()

const form = reactive({
  id: null as number | null,
  date: '',
  time: '',
  duration_mins: null as number | null,
  session_type_id: null as number | null,
  trainer_id: null as number | null,
  location_id: null as number | null,
  cost: null as number | null,
  max_bookings_enabled: false,
  max_bookings: null as number | null,
  notes: null as string | null,
  booking_deadline_hours: 0,
  booking_deadline_mins: 0
})

const outcome = ref<{ message: string; isError: boolean } | null>(null)
const sessionTypes = ref<SessionType[]>([])
const trainers = ref<UserSummary[]>([])
const locations = ref<SessionLocation[]>([])

const selectedType = computed(() => sessionTypes.value.find((t) => t.id === form.session_type_id) ?? null)

function isNonNegative(value: number | null): boolean {
  return value !== null && !Number.isNaN(value) && value >= 0
}

const errors = computed(() => ({
  date: form.date ? null : 'Date is required',
  time: form.time ? null : 'Time is required',
  session_type_id: form.session_type_id !== null ? null : 'Session type must be selected',
  trainer_id:
    selectedType.value?.requires_trainer && form.trainer_id === null
      ? `Sessions of type '${selectedType.value.name}' require a trainer to be selected.`
      : null,
  duration_mins: isNonNegative(form.duration_mins) ? null : 'Duration must be positive',
  max_bookings:
    !form.max_bookings_enabled || isNonNegative(form.max_bookings) ? null : 'Max bookings must be a non-negative number if enabled',
  cost: isNonNegative(form.cost) ? null : 'Cost must be a non-negative number'
}))

const ready = computed(() => Object.values(errors.value).every((error) => error === null))

const deadlineText = computed(() => {
  if (!form.date || !form.time) {
    return null
  }
  const deadline = new Date(form.date + ' ' + form.time)
  deadline.setHours(deadline.getHours() - form.booking_deadline_hours)
  deadline.setMinutes(deadline.getMinutes() - form.booking_deadline_mins)
  return deadline.toLocaleDateString('en-GB', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  })
})

function onChangeSessionType() {
  form.cost = selectedType.value?.cost ?? 0
}

function pad(value: number, width = 2): string {
  return String(value).padStart(width, '0')
}

// Fills the form from an existing session; keepId false makes it a copy
function fillFrom(session: Session, keepId: boolean) {
  const datetime = new Date(session.datetime)
  form.id = keepId ? session.id : null
  form.date = `${pad(datetime.getFullYear(), 4)}-${pad(datetime.getMonth() + 1)}-${pad(datetime.getDate())}`
  form.time = `${pad(datetime.getHours())}:${pad(datetime.getMinutes())}`
  form.duration_mins = session.duration_mins
  form.session_type_id = session.session_type.id
  form.trainer_id = session.trainer?.id ?? null
  form.location_id = session.location?.id ?? null
  form.cost = session.cost
  form.max_bookings_enabled = session.max_booking_count !== null
  form.max_bookings = session.max_booking_count
  form.notes = session.notes
  form.booking_deadline_hours = Math.floor(session.booking_deadline_duration_mins / 60)
  form.booking_deadline_mins = session.booking_deadline_duration_mins % 60
}

// Fills the form from session `sourceId`, to edit it or as the template for a copy
async function loadSession(sourceId: number, edit: boolean) {
  const session = await tryApi(() => getSession(sourceId))
  if (session) {
    fillFrom(session, edit)
  }
}

// ?edit=<id> opens a session for editing, ?copy=<id> pre-fills a new one
async function loadFromQuery() {
  const editId = query.get('edit')
  const copyId = query.get('copy')
  const sourceId = Number(editId ?? copyId)
  if (sourceId) {
    await loadSession(sourceId, editId !== null)
  }
}

// Keeps the URL in step with what the form holds, so it can be reloaded
function showInQuery(sourceId: number, edit: boolean) {
  query.update({ edit: edit ? String(sourceId) : null, copy: edit ? null : String(sourceId) })
}

function toRequest(): NewSession {
  return {
    datetime: new Date(form.date + ' ' + form.time).toISOString(),
    duration_mins: form.duration_mins ?? 0,
    session_type_id: form.session_type_id ?? 0,
    location_id: form.location_id,
    trainer_id: form.trainer_id,
    cost: form.cost ?? 0,
    max_bookings: form.max_bookings_enabled ? form.max_bookings : null,
    notes: form.notes,
    booking_deadline_mins: form.booking_deadline_hours * 60 + form.booking_deadline_mins
  }
}

async function save(createAnother: boolean) {
  outcome.value = null
  const request = toRequest()
  if (form.id) {
    const done = await tryApi(async () => {
      await updateSession(form.id!, request)
      return true
    })
    if (!done) {
      return
    }
    if (createAnother) {
      showInQuery(form.id, false)
      await loadSession(form.id, false)
    } else {
      outcome.value = { message: 'Saved!', isError: false }
    }
  } else {
    const newId = await tryApi(() => createSession(request))
    if (newId === undefined) {
      return
    }
    // Reload the new session, in edit mode or as the template for the next one
    showInQuery(newId, !createAnother)
    await loadSession(newId, !createAnother)
  }
}

async function loadReferenceData() {
  const [types, trainerList, locationList] = await Promise.all([
    tryApi(() => listSessionTypes(false)),
    tryApi(() => listUsers({ role: 'trainer' })),
    tryApi(() => listLocations())
  ])
  sessionTypes.value = types ?? []
  trainers.value = trainerList ?? []
  locations.value = locationList ?? []
}

watch(form, () => {
  outcome.value = null
})

void loadReferenceData().then(loadFromQuery)
</script>
