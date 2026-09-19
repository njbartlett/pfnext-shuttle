<template>
  <div v-if="user" class="mt-2">
    <!-- Member has booked -->
    <span v-if="session.booked" class="me-1">
      <!-- Past session: attendance status and feedback -->
      <span v-if="isPastAt(session.datetime, now)">
        <span v-if="session.attended" class="text-success text-align-middle">
          <i class="bi bi-check2-circle"></i>&nbsp;Attended
          &nbsp;
          <button class="btn btn-sm btn-outline-primary" type="button" title="Give feedback" @click="emit('feedback', session)"><i class="bi bi-chat-quote"></i></button>
        </span>
        <span v-else class="text-danger"><i class="bi bi-x-circle"></i>&nbsp;Didn't Attend</span>
      </span>
      <!-- Future session: booked, may cancel -->
      <div v-else class="btn-group">
        <button type="button" class="btn btn-success btn-sm dropdown-toggle" data-bs-toggle="dropdown" aria-expanded="false"><i class="bi bi-check2-circle"></i>&nbsp;Booked!</button>
        <ul class="dropdown-menu">
          <li><a class="dropdown-item link-danger" href="#" @click.prevent="emit('cancel', session)"><i class="bi bi-x-circle"></i>&nbsp;Cancel</a></li>
        </ul>
      </div>
    </span>

    <!-- Not booked, session in the future -->
    <span v-else-if="!isPastAt(session.datetime, now)" class="me-1">
      <span v-if="isPastAt(session.booking_deadline, now)" class="text-danger"><i class="bi bi-sign-stop"></i>&nbsp;Deadline Passed!</span>
      <span v-else>
        <!-- Full: waitlist -->
        <span v-if="isFull(session)">
          <div v-if="session.waitlist_rank" class="btn-group">
            <button type="button" class="btn btn-outline-warning btn-sm dropdown-toggle" data-bs-toggle="dropdown" aria-expanded="false">
              <i class="bi bi-hourglass-split"></i>&nbsp;On Waitlist&nbsp;({{ session.waitlist_rank }})
            </button>
            <ul class="dropdown-menu">
              <li><a class="dropdown-item link-danger" href="#" @click.prevent="emit('leave-waitlist', session)"><i class="bi bi-x-circle"></i>&nbsp;Leave Waitlist</a></li>
            </ul>
          </div>
          <button v-else type="button" class="btn btn-warning btn-sm" @click="emit('join-waitlist', session)">Join Waitlist</button>
        </span>
        <button v-else type="button" class="btn btn-primary btn-sm" @click="emit('book', session)">
          <span>Book</span><span v-if="isSoon(session.booking_deadline, DEADLINE_WARNING_MINUTES, now)">&nbsp;within {{ timeLeft }}</span>
        </button>
      </span>
    </span>

    <!-- Unbooked past sessions show nothing -->

    <!-- Admin dropdown -->
    <div v-if="isAdmin" class="btn-group dropup">
      <button type="button" class="btn btn-outline-primary btn-sm dropdown-toggle" data-bs-toggle="dropdown" aria-expanded="false"><i class="bi bi-gear-fill"></i></button>
      <ul class="dropdown-menu">
        <li><RouterLink class="dropdown-item" :to="toolRoute('attendance', { id: session.id })"><i class="bi bi-list-check"></i> Attendance</RouterLink></li>
        <li><RouterLink class="dropdown-item" :to="toolRoute('feedback', { id: session.id })"><i class="bi bi-chat-text"></i> Feedback</RouterLink></li>
        <li><RouterLink class="dropdown-item" :to="toolRoute('edit_session', { edit: session.id })"><i class="bi bi-pencil"></i> Edit</RouterLink></li>
        <li><RouterLink class="dropdown-item" :to="toolRoute('edit_session', { copy: session.id })"><i class="bi bi-copy"></i> Copy</RouterLink></li>
        <li><button type="button" class="dropdown-item text-danger" @click="emit('delete', session)"><i class="bi bi-trash"></i> Delete</button></li>
      </ul>
    </div>
    <RouterLink v-else-if="isSessionTrainer" class="btn btn-outline-primary btn-sm" :to="toolRoute('attendance', { id: session.id })"><i class="bi bi-list-check"></i> Attendance</RouterLink>
  </div>
</template>

<script setup lang="ts">
// Book / cancel / waitlist / feedback controls for one session, plus the
// admin menu. Replaces the session_controls Tera include. The parent owns
// the API calls and confirmation dialogs; this component only emits.
import { computed } from 'vue'
import { useRoute, type RouteLocationRaw } from 'vue-router'
import { formatDuration, isFull, type Session } from '@pfnext/shared'
import { isPastAt, isSoon } from '@/composables/useNow'
import { isAdmin, isTrainer, user } from '@/stores/auth'

const DEADLINE_WARNING_MINUTES = 180

const props = defineProps<{ session: Session; now: Date }>()
const route = useRoute()

// The trainer/admin tools return to this page (and its week) when closed
function toolRoute(name: string, query: Record<string, number>): RouteLocationRaw {
  return { name, query: { ...query, return: route.fullPath } }
}
const emit = defineEmits<{
  book: [session: Session]
  cancel: [session: Session]
  'join-waitlist': [session: Session]
  'leave-waitlist': [session: Session]
  feedback: [session: Session]
  delete: [session: Session]
}>()

const isSessionTrainer = computed(
  () => isTrainer.value && props.session.trainer !== null && props.session.trainer.id === user.value?.id
)

const timeLeft = computed(() =>
  formatDuration(new Date(props.session.booking_deadline).getTime() - props.now.getTime())
)
</script>
