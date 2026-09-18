<template>
  <div v-if="session" class="card my-2">
    <div class="card-header d-flex align-items-center">
      <svg xmlns="http://www.w3.org/2000/svg" width="50" height="50" fill="currentColor" class="bi bi-sun-fill" viewBox="0 0 54 54">
        <path d="m 29.64,32.02 2.7,-4.68 8.16,14.04 h 5.4 C 44.7,39.46 33.3,19.96 32.34,18.04 L 20.58,38.44 16.2,46.06 H 0 C 0.9,44.56 6.96,34 12.78,23.86 16.2,17.92 19.98,11.26 24.24,4 h 5.4 L 18.48,23.32 8.1,41.38 h 5.4 C 20.88,28.36 25.5,20.68 32.34,8.68 32.76,9.4 53.88,46 54,46.06 H 37.8 Z"/>
      </svg>
      <h2 class="ms-2">Session Attendance</h2>
      <div class="ms-auto align-middle">
        <button type="button" class="btn-close" aria-label="Close" @click="goBack()"></button>
      </div>
    </div>
    <ul class="list-group list-group-flush">
      <li class="list-group-item">
        <h2>Bookings</h2>
        <p>
          {{ bookings.length }} member(s) booked {{ session.session_type.name }} session at {{ displayVenueTime(session.datetime) }} on {{ displayFullDate(session.datetime) }}<span v-if="session.location"> at {{ session.location.name }}</span>:
        </p>
        <table class="table table-hover table-sm">
          <thead>
            <tr>
              <th class="align-middle">
                <span>Member</span>
                <SortButtons field="person_name" :active="sort.field === 'person_name'" :ascending="sort.ascending" @sort="applySort" />
              </th>
              <th>
                <span>Attended?</span>
                <SortButtons field="attended" :active="sort.field === 'attended'" :ascending="sort.ascending" @sort="applySort" />
              </th>
              <th>
                <span>Booked At</span>
                <SortButtons field="booked_timestamp" :active="sort.field === 'booked_timestamp'" :ascending="sort.ascending" @sort="applySort" />
              </th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="booking in bookings" :key="booking.person_id" class="align-middle">
              <td>
                <BibIcon :status="booking.person_status"><span class="icon d-inline-block"></span></BibIcon>
                &nbsp;{{ booking.person_name }}
              </td>
              <td>
                <input type="checkbox" class="form-check larger align-middle" :checked="booking.attended" @change="toggleAttendance(booking)">
              </td>
              <td>{{ displayDateTime(booking.booked_timestamp) }}</td>
              <td>
                <button type="button" class="btn btn-outline-danger btn-sm float-end" title="Remove booking" @click="removeMember(booking.person_id)"><i class="bi bi-trash-fill"></i></button>
              </td>
            </tr>
          </tbody>
        </table>

        <div class="btn-group my-2 float-end" role="group" aria-label="Bulk Attendance Operations">
          <button type="button" class="btn btn-primary" @click="setAllAttended(true)"><i class="bi bi-check-all"></i>&nbsp;All Attended</button>
          <button type="button" class="btn btn-outline-danger" @click="setAllAttended(false)"><i class="bi bi-x-square"></i>&nbsp;Clear All</button>
        </div>

        <form @submit.prevent="addMember">
          <div class="input-group m-0 p-0">
            <span class="input-group-text">+</span>
            <MemberPicker ref="picker" v-model="addingMember" :members="members" />
            <button type="submit" class="btn btn-primary" :disabled="!addingMember"><i class="bi bi-arrow-up-square-fill"></i>&nbsp;Book</button>
          </div>
        </form>
      </li>

      <li v-if="waitlist.length > 0" class="list-group-item">
        <h2 class="my-2">Waitlist</h2>
        <table class="table table-hover table-sm">
          <thead>
            <tr>
              <th>&num;</th>
              <th>Member</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="entry in waitlist" :key="entry.id" class="align-middle">
              <td>{{ entry.rank }}</td>
              <td width="100%">
                <BibIcon :status="entry.person_status"><span class="icon d-inline-block"></span></BibIcon>
                &nbsp;{{ entry.person_name }}
              </td>
              <td>
                <button type="button" class="btn btn-outline-danger btn-sm" title="Remove" @click="removeWaitlistEntry(entry)"><i class="bi bi-trash-fill"></i></button>
              </td>
            </tr>
          </tbody>
        </table>
      </li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { reactive, ref } from 'vue'
import {
  addBooking, cancelBooking, displayDateTime, displayFullDate, displayVenueTime, getSession, leaveWaitlist,
  listSessionBookings, listWaitlist, setAttendance, sortByField,
  type Booking, type Session, type UserSummary, type WaitlistEntry
} from '@pfnext/shared'
import BibIcon from '@/components/BibIcon.vue'
import MemberPicker from '@/components/MemberPicker.vue'
import SortButtons from '@/components/SortButtons.vue'
import { useReturnPath } from '@/composables/useReturnPath'
import { loadSelectableMembers } from '@/composables/useSelectableMembers'
import { urlQuery } from '@/composables/useUrlState'
import { reportApiError, tryApi } from '@/stores/apiError'

const { goBack } = useReturnPath('/sessions.html')

const session = ref<Session | null>(null)
const bookings = ref<Booking[]>([])
const waitlist = ref<WaitlistEntry[]>([])
const members = ref<UserSummary[]>([])
const addingMember = ref<UserSummary | null>(null)
const picker = ref<InstanceType<typeof MemberPicker> | null>(null)
const sort = reactive<{ field: keyof Booking; ascending: boolean }>({ field: 'person_name', ascending: true })

const sessionId = Number(urlQuery().get('id'))

function sortBookings() {
  sortByField(bookings.value, sort.field, sort.ascending)
}

function applySort(field: string, ascending: boolean) {
  sort.field = field as keyof Booking
  sort.ascending = ascending
  sortBookings()
}

async function loadBookings() {
  const result = await tryApi(() => listSessionBookings(sessionId))
  if (result) {
    bookings.value = result
    sortBookings()
  }
}

async function loadWaitlist() {
  waitlist.value = (await tryApi(() => listWaitlist(sessionId))) ?? []
}

async function addMember() {
  if (!addingMember.value) {
    return
  }
  const done = await tryApi(async () => {
    await addBooking(addingMember.value!.id, sessionId)
    return true
  })
  if (done) {
    picker.value?.clear()
    await loadBookings()
  }
}

async function removeMember(personId: number) {
  const done = await tryApi(async () => {
    await cancelBooking(personId, sessionId)
    return true
  })
  if (done) {
    await Promise.all([loadBookings(), loadWaitlist()])
  }
}

async function toggleAttendance(booking: Booking) {
  const attended = !booking.attended
  const done = await tryApi(async () => {
    await setAttendance(sessionId, booking.person_id, attended)
    return true
  })
  if (done) {
    booking.attended = attended
    sortBookings()
  }
}

async function setAllAttended(attended: boolean) {
  const done = await tryApi(async () => {
    await setAttendance(sessionId, null, attended)
    return true
  })
  if (done) {
    for (const booking of bookings.value) {
      booking.attended = attended
    }
    sortBookings()
  }
}

async function removeWaitlistEntry(entry: WaitlistEntry) {
  const done = await tryApi(async () => {
    await leaveWaitlist(entry.person_id, entry.session_id)
    return true
  })
  if (done) {
    await Promise.all([loadBookings(), loadWaitlist()])
  }
}

if (sessionId) {
  void tryApi(() => getSession(sessionId)).then((result) => {
    session.value = result ?? null
  })
  void loadBookings()
  void loadWaitlist()
  void loadSelectableMembers().then((result) => {
    members.value = result
  })
} else {
  reportApiError(new Error('missing session id'))
}
</script>
