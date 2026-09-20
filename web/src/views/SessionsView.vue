<template>
  <div class="container">
    <PageTitle />

    <!-- Prompt to provide emergency contact info -->
    <div v-if="userRecord && (!userRecord.emergency_name || !userRecord.emergency_phone)" class="alert alert-warning" role="alert">
      <i class="bi bi-exclamation-triangle-fill"></i>&nbsp;Please <RouterLink to="/profile.html" class="alert-link">provide an emergency contact</RouterLink>.
    </div>

    <!-- Open polls -->
    <div v-for="entry in unvotedPolls" :key="entry.poll.id" class="alert alert-info" role="alert">
      <i class="bi bi-info-circle"></i>&nbsp;There is an open poll: <RouterLink :to="{ path: '/polls.html', hash: '#poll-' + entry.poll.id }" class="alert-link">{{ entry.poll.question }}</RouterLink>
    </div>

    <div v-if="!user" class="alert alert-light" role="alert">
      You are not logged in.
      <RouterLink :to="loginRoute(route.fullPath)" role="button" class="btn btn-success rounded-pill mx-2">Login to Book</RouterLink>
    </div>

    <!-- Weekly pagination -->
    <PagerBar
      :indices="pager.indices.value"
      :offset="pager.offset.value"
      :home="pager.home.value"
      :label="weekLabel"
      @select="selectWeek"
      @back="pager.back()"
      @forward="pager.forward()"
      @reset="resetWeek"
    >
      <button v-if="viewMode === 'list'" type="button" class="btn btn-outline-primary ms-1" title="Switch to Calendar View" @click="viewMode = 'cal'"><i class="bi bi-calendar3"></i></button>
      <button v-else type="button" class="btn btn-outline-primary ms-1" title="Switch to List View" @click="viewMode = 'list'"><i class="bi bi-list-ul"></i></button>
    </PagerBar>

    <!-- List view -->
    <table v-if="viewMode === 'list'" class="table table-borderless table-sm align-middle">
      <thead>
        <tr>
          <th scope="col">When</th>
          <th scope="col">Trainer</th>
          <th scope="col">What &amp; Where</th>
          <th v-if="user" scope="col">Actions</th>
        </tr>
      </thead>
      <tbody v-for="session in sessions" :key="session.id">
        <tr class="border-top" :class="{ 'session-past': isPast(session.datetime), 'border-bottom': !session.notes }">
          <td :rowspan="session.notes ? 2 : 1">{{ displayFullDate(session.datetime) }} {{ displayVenueTime(session.datetime) }}</td>
          <td :rowspan="session.notes ? 2 : 1">
            <div v-if="session.trainer">
              <a v-if="session.trainer.url" :href="session.trainer.url" :class="{ 'link-secondary': isPast(session.datetime) }"><i class="bi bi-person-arms-up"></i>&nbsp;{{ session.trainer.name }}</a>
              <span v-else><i class="bi bi-person-arms-up"></i>&nbsp;{{ session.trainer.name }}</span>
            </div>
            <div v-else>N/A</div>
          </td>
          <td>
            <span>{{ session.session_type.name }}</span>
            <span v-if="session.location">,
              <a v-if="session.location.url" :href="session.location.url" :class="{ 'link-secondary': isPast(session.datetime) }">{{ session.location.name }}</a>
              <span v-else>{{ session.location.name }}</span>
            </span>
            <BookingCountPill :session="session" />
            <SessionRatings :session="session" />
          </td>
          <td v-if="user">
            <SessionControls :session="session" :now="now" v-bind="controlHandlers" />
          </td>
        </tr>
        <tr v-if="session.notes" class="border-bottom" :class="{ 'session-past': isPast(session.datetime) }">
          <td colspan="4" v-html="session.notes"></td>
        </tr>
      </tbody>
    </table>

    <!-- Calendar view -->
    <table v-else class="table table-sm table-borderless">
      <thead>
        <tr class="border-top">
          <th v-for="day in daysOfWeek" :key="day.index" scope="col" class="border-end bg-light-subtle" :class="{ 'bg-opacity-50': !day.isToday, 'border-start': day.index === 0 }">
            <span class="d-block d-sm-none">{{ displayCalendarDate(day.date, 'xs') }}</span>
            <span class="d-none d-sm-block d-lg-none">{{ displayCalendarDate(day.date, 's') }}</span>
            <span class="d-none d-lg-block">{{ displayCalendarDate(day.date, 'l') }}</span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="hour in sessionsByHour" :key="hour.time">
          <td v-for="day in hour.days" :key="day.index" class="border-start border-end" :class="{ 'bg-light-subtle': day.isToday }">
            <div
              v-for="session in day.sessions"
              :key="session.id"
              class="border rounded p-1 hover-expand"
              :class="isPast(session.datetime) ? 'bg-secondary-subtle text-secondary' : 'border-primary bg-primary-subtle'"
            >
              <div class="d-flex align-items-center mb-1">
                <strong>{{ displayVenueTime(session.datetime) }} {{ session.session_type.name }}</strong>
                <BookingCountPill :session="session" class="ms-auto" />
              </div>
              <div v-if="session.location">
                <a v-if="session.location.url" :href="session.location.url" :class="{ 'link-secondary': isPast(session.datetime) }"><i class="bi bi-geo-alt-fill"></i>&nbsp;{{ session.location.name }}</a>
                <span v-else><i class="bi bi-geo-alt-fill"></i>&nbsp;{{ session.location.name }}</span>
              </div>
              <div v-if="session.trainer">
                <a v-if="session.trainer.url" :href="session.trainer.url" :class="{ 'link-secondary': isPast(session.datetime) }"><i class="bi bi-person-arms-up"></i>&nbsp;{{ session.trainer.name }}</a>
                <span v-else><i class="bi bi-person-arms-up"></i>&nbsp;{{ session.trainer.name }}</span>
              </div>
              <SessionRatings :session="session" />
              <div v-if="session.notes" class="text-wrap" style="max-width: 15rem;" v-html="session.notes"></div>
              <SessionControls :session="session" :now="now" v-bind="controlHandlers" />
            </div>
          </td>
        </tr>
        <tr class="border-bottom"></tr>
      </tbody>
    </table>

    <p class="text-center text-secondary">
      Showing week commencing {{ displayFullDate(weekStart) }}. Found {{ sessions.length }} session(s) for selected week. Session times shown are local to the venue.
    </p>

    <!-- Delete confirmation -->
    <ConfirmModal ref="deleteModal" title="Delete Session" icon="bi-exclamation-triangle-fill" header-class="bg-danger" confirm-label="Delete" confirm-icon="bi-trash" confirm-class="btn-danger" @confirm="deleteSelected">
      <template v-if="deletingSession">
        <p>Are you sure you want to delete the following session?</p>
        <ul>
          <li>{{ displayFullDate(deletingSession.datetime) }} at {{ displayVenueTime(deletingSession.datetime) }}</li>
          <li>{{ deletingSession.session_type.name }}<span v-if="deletingSession.location">, {{ deletingSession.location.name }}</span></li>
          <li v-if="deletingSession.trainer">{{ deletingSession.trainer.name }}</li>
          <li>{{ deletingSession.booking_count }} member(s) booked</li>
        </ul>
        <p>This cannot be undone. All related bookings and attendance records will be deleted.</p>
      </template>
    </ConfirmModal>

    <!-- Spend credits confirmation -->
    <ConfirmModal ref="creditsModal" title="Book with Credits" icon="bi-exclamation-triangle-fill" centered @confirm="confirmCredits">
      <template v-if="paymentConfirm">
        <p class="fs-5">Do you want to use {{ paymentConfirm.cost }} of your pay-as-you-go credits to book this session?</p>
        <p class="text-secondary">
          <i class="bi bi-info-circle"></i>&nbsp;<span v-if="paymentConfirm.cost === 1">This credit</span><span v-else>These credits</span> will be refunded if you cancel the booking before the session begins.
        </p>
      </template>
    </ConfirmModal>

    <!-- Joined waitlist -->
    <BsModal ref="waitlistModal" dialog-class="modal-dialog-centered">
      <div class="modal-header bg-warning text-bg-warning">
        <h5 class="modal-title"><i class="bi bi-hourglass-split"></i>&nbsp;Joined Waitlist</h5>
        <button type="button" class="btn-close text-bg-warning" data-bs-dismiss="modal" aria-label="Close"></button>
      </div>
      <div v-if="waitlistJoined" class="modal-body">
        <p class="text-center fs-5">You have joined the waiting list at position</p>
        <p class="text-center fs-1">{{ waitlistJoined.rank }}</p>
        <hr>
        <p class="text-secondary">If a space becomes available for you, it will be booked automatically and you will be notified by email.</p>
        <p v-if="waitlistJoined.cost > 0" class="text-secondary">
          <strong>NB:</strong> You must have a valid membership or sufficient Pay As You Go credits, otherwise the booking will fail and you will lose your waiting list place.
        </p>
      </div>
      <div class="modal-footer">
        <button type="button" class="btn btn-primary" data-bs-dismiss="modal">Got it!</button>
      </div>
    </BsModal>

    <!-- Session feedback -->
    <BsModal ref="feedbackModal" dialog-class="modal-dialog-centered">
      <div class="modal-header bg-primary">
        <h5 class="modal-title">Your Feedback</h5>
        <button type="button" class="btn-close" data-bs-dismiss="modal" aria-label="Close"></button>
      </div>
      <div class="modal-body">
        <p v-if="feedback.session">
          Please provide your feedback for {{ feedback.session.session_type.name }} session
          <span v-if="feedback.session.trainer">led by {{ feedback.session.trainer.name }}</span>
          on {{ displayFullDate(feedback.session.datetime) }} at {{ displayVenueTime(feedback.session.datetime) }}.
        </p>
        <form @submit.prevent="saveSessionFeedback">
          <label for="sessionRating" class="form-label">Rating:</label>
          <select id="sessionRating" v-model="feedback.rating" class="form-control">
            <option :value="5">★★★★★ 😁</option>
            <option :value="4">★★★★ 😀</option>
            <option :value="3">★★★ 😐</option>
            <option :value="2">★★ 🙁</option>
            <option :value="1">★ 😥</option>
          </select>
          <label for="sessionFeedbackComment" class="form-label">Comments:</label>
          <textarea id="sessionFeedbackComment" v-model="feedback.comment" class="form-control" rows="5"></textarea>
        </form>
        <div class="alert alert-secondary my-2">
          <i class="bi bi-info-circle"></i>&nbsp;Ratings and comments can be viewed anonymously by the session trainer and by admins. If you would like your name to be known, sign your comment.
        </div>
        <div v-if="feedback.error" class="alert alert-danger my-2"><i class="bi bi-exclamation-triangle-fill"></i> {{ feedback.error }}</div>
      </div>
      <div class="modal-footer">
        <button type="button" class="btn btn-secondary" data-bs-dismiss="modal">Close</button>
        <button type="button" class="btn btn-primary" @click="saveSessionFeedback">Save changes</button>
      </div>
    </BsModal>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import {
  addDays, bookSession, cancelBooking, deleteSession, displayDateRange, displayFullDate, displayVenueTime,
  getUserRecord, isPast, joinWaitlist, leaveWaitlist, listPolls, listSessions, saveFeedback, startOfWeek,
  type PollWithVotes, type Session, type UserRecord
} from '@pfnext/shared'
import BookingCountPill from '@/components/BookingCountPill.vue'
import BsModal from '@/components/BsModal.vue'
import ConfirmModal from '@/components/ConfirmModal.vue'
import PagerBar from '@/components/PagerBar.vue'
import PageTitle from '@/components/PageTitle.vue'
import SessionControls from '@/components/SessionControls.vue'
import SessionRatings from '@/components/SessionRatings.vue'
import { useNow } from '@/composables/useNow'
import { usePagedWindow } from '@/composables/usePagedWindow'
import { useQueryState } from '@/composables/useQueryState'
import { loginRoute } from '@/router'
import { user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const MILLIS_IN_WEEK = 7 * 24 * 60 * 60 * 1000
const VIEW_MODE_STORAGE_KEY = 'view_mode'
const HOURS = Array.from({ length: 24 }, (_, hour) => String(hour).padStart(2, '0') + ':00')

type ViewMode = 'list' | 'cal'

const route = useRoute()
const now = useNow()
const pager = usePagedWindow()
const query = useQueryState()

const sessions = ref<Session[]>([])
const userRecord = ref<UserRecord | null>(null)
const polls = ref<PollWithVotes[]>([])
const viewMode = ref<ViewMode>(readViewMode())

const deletingSession = ref<Session | null>(null)
const paymentConfirm = ref<{ session: Session; cost: number } | null>(null)
const waitlistJoined = ref<{ rank: number; cost: number } | null>(null)
const feedback = reactive<{ session: Session | null; rating: number | null; comment: string; error: string | null }>({
  session: null,
  rating: null,
  comment: '',
  error: null
})

const deleteModal = ref<InstanceType<typeof ConfirmModal> | null>(null)
const creditsModal = ref<InstanceType<typeof ConfirmModal> | null>(null)
const waitlistModal = ref<InstanceType<typeof BsModal> | null>(null)
const feedbackModal = ref<InstanceType<typeof BsModal> | null>(null)

const currentWeekStart = startOfWeek(new Date())
const weekStart = computed(() => addDays(currentWeekStart, 7 * pager.offset.value))
const weekEnd = computed(() => addDays(weekStart.value, 7))

const unvotedPolls = computed(() => polls.value.filter((entry) => entry.poll.open && entry.votes.length === 0))

interface CalendarDay {
  index: number
  date: Date
  isToday: boolean
  sessions: Session[]
}

const daysOfWeek = computed<CalendarDay[]>(() => {
  const today = new Date()
  return Array.from({ length: 7 }, (_, index) => {
    const date = addDays(weekStart.value, index)
    return { index, date, isToday: isSameDay(date, today), sessions: [] }
  })
})

// Calendar grid: one row per hour, one cell per day
const sessionsByHour = computed(() => {
  const rows = HOURS.map((time) => ({
    time,
    days: daysOfWeek.value.map((day) => ({ ...day, sessions: [] as Session[] }))
  }))
  for (const session of sessions.value) {
    const datetime = new Date(session.datetime)
    const hour = displayVenueTime(datetime).split(':')[0]
    const row = rows.find((r) => r.time === hour + ':00')
    const dayIndex = (7 + datetime.getDay() - currentWeekStart.getDay()) % 7
    row?.days[dayIndex]?.sessions.push(session)
  }
  return rows
})

function isSameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
}

function readViewMode(): ViewMode {
  try {
    return localStorage.getItem(VIEW_MODE_STORAGE_KEY) === 'list' ? 'list' : 'cal'
  } catch {
    return 'cal'
  }
}

function weekLabel(offset: number): string {
  if (offset === -1) return 'Last Week'
  if (offset === 0) return 'This Week'
  if (offset === 1) return 'Next Week'
  const start = addDays(currentWeekStart, offset * 7)
  return displayDateRange(start, addDays(start, 6))
}

function displayCalendarDate(date: Date, length: 'xs' | 's' | 'l'): string {
  const options: Intl.DateTimeFormatOptions =
    length === 'xs' ? { weekday: 'narrow' } : length === 's' ? { weekday: 'short' } : { weekday: 'short', day: 'numeric', month: 'short' }
  return date.toLocaleDateString(undefined, options)
}

async function loadSessions() {
  const result = await tryApi(() => listSessions(weekStart.value, weekEnd.value))
  if (result) {
    sessions.value = result
  }
}

async function loadUser() {
  userRecord.value = user.value ? ((await tryApi(() => getUserRecord(user.value!.id))) ?? null) : null
}

async function loadPolls() {
  if (!user.value) {
    polls.value = []
    return
  }
  polls.value = (await tryApi(() => listPolls({ personId: user.value!.id }))) ?? []
}

function selectWeek(offset: number) {
  pager.offset.value = offset
}

function resetWeek() {
  pager.reset()
}

// Booking actions, passed to every SessionControls as listeners
const controlHandlers = {
  onBook: (session: Session) => book(session, 0),
  onCancel: cancel,
  onJoinWaitlist: join,
  onLeaveWaitlist: leave,
  onFeedback: openFeedback,
  onDelete: (session: Session) => {
    deletingSession.value = session
    deleteModal.value?.show()
  }
}

async function book(session: Session, creditsUsed: number) {
  const result = await tryApi(() => bookSession(user.value!.id, session.id, creditsUsed, session.cost))
  if (!result) {
    return
  }
  if (result.outcome === 'credits_required') {
    paymentConfirm.value = { session, cost: result.cost }
    creditsModal.value?.show()
  } else {
    session.booking_count++
    session.booked = true
  }
}

function confirmCredits() {
  if (paymentConfirm.value) {
    void book(paymentConfirm.value.session, paymentConfirm.value.cost)
  }
}

async function cancel(session: Session) {
  const done = await tryApi(async () => {
    await cancelBooking(user.value!.id, session.id)
    return true
  })
  if (done) {
    session.booking_count--
    session.booked = false
  }
}

async function join(session: Session) {
  const entry = await tryApi(() => joinWaitlist(user.value!.id, session.id))
  if (entry) {
    waitlistJoined.value = { rank: entry.rank, cost: session.cost }
    waitlistModal.value?.show()
    await loadSessions()
  }
}

async function leave(session: Session) {
  const done = await tryApi(async () => {
    await leaveWaitlist(user.value!.id, session.id)
    return true
  })
  if (done) {
    await loadSessions()
  }
}

async function deleteSelected() {
  if (!deletingSession.value) {
    return
  }
  const done = await tryApi(async () => {
    await deleteSession(deletingSession.value!.id)
    return true
  })
  if (done) {
    await loadSessions()
  }
}

function openFeedback(session: Session) {
  feedback.session = session
  feedback.rating = session.rating
  feedback.comment = session.comment ?? ''
  feedback.error = null
  feedbackModal.value?.show()
}

async function saveSessionFeedback() {
  if (!feedback.session || feedback.rating === null) {
    feedback.error = 'Please choose a rating'
    return
  }
  try {
    await saveFeedback(feedback.session.id, user.value!.id, { rating: feedback.rating, comment: feedback.comment })
    feedbackModal.value?.hide()
    await loadSessions()
  } catch (error) {
    feedback.error = error instanceof Error ? error.message : String(error)
  }
}

// A bookmarked or emailed ?week= opens on that week. Done before the offset
// watcher is registered so the initial load below is the only one.
const weekParam = query.get('week')
if (weekParam) {
  const targetWeek = startOfWeek(new Date(weekParam))
  pager.jumpTo(Math.floor((targetWeek.getTime() - currentWeekStart.getTime()) / MILLIS_IN_WEEK))
}

// Keep the selected week in the URL so it can be bookmarked
watch(
  () => pager.offset.value,
  (offset) => {
    query.set('week', offset === 0 ? null : weekStart.value.toLocaleDateString('sv'))
    void loadSessions()
  }
)

watch(viewMode, (mode) => {
  try {
    localStorage.setItem(VIEW_MODE_STORAGE_KEY, mode)
  } catch {
    // Preference simply not remembered
  }
})

watch(user, () => {
  void loadUser()
  void loadPolls()
})

void loadSessions()
void loadUser()
void loadPolls()

// Re-check polls when the page is restored from the back/forward cache
const onPageReveal = () => void loadPolls()
window.addEventListener('pagereveal', onPageReveal)
onBeforeUnmount(() => window.removeEventListener('pagereveal', onPageReveal))
</script>
