<template>
  <div class="container">
    <PageTitle />

    <AdminPanel>
      <form @submit.prevent>
        <label for="selectMemberDataList" class="form-label">Show Bookings for Member:</label>
        <MemberPicker v-model="selectedMember" input-id="selectMemberDataList" :members="members" />
      </form>
    </AdminPanel>

    <PagerBar
      :indices="pager.indices.value"
      :offset="pager.offset.value"
      :home="pager.home.value"
      :label="monthLabel"
      @select="selectMonth"
      @back="pager.back()"
      @forward="pager.forward()"
      @reset="resetMonth"
    />

    <p class="text-secondary">
      Showing bookings for {{ selectedMember ? selectedMember.name : 'N/A' }} in {{ monthLabel(pager.offset.value) }}.
    </p>
    <table class="table table-hover table-sm">
      <thead>
        <tr>
          <th scope="col">Date &amp; Time</th>
          <th scope="col">Type</th>
          <th scope="col">Location</th>
          <th scope="col"></th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="booking in bookings" :key="booking.session_id">
          <td>{{ displayFullDate(booking.session_datetime) }} at {{ displayVenueTime(booking.session_datetime) }}</td>
          <td>{{ booking.session_type.name }}</td>
          <td>
            <a v-if="booking.session_location" :href="'http://www.google.com/maps/search/' + encodeURIComponent(booking.session_location.address)" target="_blank">{{ booking.session_location.name }}</a>
          </td>
          <td v-if="isPast(booking.session_datetime)">
            <span v-if="booking.attended" class="text-success"><i class="bi bi-check2-circle"></i> Attended</span>
            <span v-else class="text-danger"><i class="bi bi-x-circle"></i> Didn't Attend</span>
          </td>
          <td v-else>
            <span v-if="booking.credits_used > 0" class="text-success" :title="booking.credits_used + ' credit(s) spent'">{{ booking.credits_used }}&nbsp;<i class="bi bi-ticket-perforated"></i>&nbsp;</span>
            <a href="#" class="btn btn-sm btn-outline-danger" @click.prevent="cancel(booking)"><i class="bi bi-trash"></i>&nbsp;Cancel</a>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import {
  addMonths, cancelBooking, displayFullDate, displayVenueTime, endOfMonth, isPast, listBookings, startOfMonth,
  type Booking, type UserSummary
} from '@pfnext/shared'
import AdminPanel from '@/components/AdminPanel.vue'
import MemberPicker from '@/components/MemberPicker.vue'
import PagerBar from '@/components/PagerBar.vue'
import PageTitle from '@/components/PageTitle.vue'
import { usePagedWindow } from '@/composables/usePagedWindow'
import { loadSelectableMembers } from '@/composables/useSelectableMembers'
import { user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const pager = usePagedWindow()
const members = ref<UserSummary[]>([])
const selectedMember = ref<UserSummary | null>(null)
const bookings = ref<Booking[]>([])

function monthStart(offset: number): Date {
  return addMonths(startOfMonth(new Date()), offset)
}

function monthLabel(offset: number): string {
  return monthStart(offset).toLocaleDateString('en-GB', { month: 'short', year: 'numeric' })
}

async function load() {
  if (!selectedMember.value) {
    bookings.value = []
    return
  }
  const start = monthStart(pager.offset.value)
  const personId = selectedMember.value.id
  const result = await tryApi(() => listBookings(personId, start, endOfMonth(start)))
  if (result) {
    bookings.value = result
  }
}

function selectMonth(offset: number) {
  pager.offset.value = offset
}

function resetMonth() {
  pager.reset()
}

async function cancel(booking: Booking) {
  const done = await tryApi(async () => {
    await cancelBooking(booking.person_id, booking.session_id)
    return true
  })
  if (done) {
    bookings.value = bookings.value.filter((b) => b !== booking)
  }
}

watch([selectedMember, () => pager.offset.value], load)

watch(
  user,
  async (current) => {
    if (!current) {
      members.value = []
      selectedMember.value = null
      return
    }
    members.value = await loadSelectableMembers()
    selectedMember.value = members.value.find((member) => member.email === current.email) ?? null
  },
  { immediate: true }
)
</script>
