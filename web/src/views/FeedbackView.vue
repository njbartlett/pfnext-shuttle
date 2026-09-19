<template>
  <div v-if="session" class="card my-2">
    <div class="card-header d-flex align-items-center">
      <svg xmlns="http://www.w3.org/2000/svg" width="50" height="50" fill="currentColor" class="bi bi-sun-fill" viewBox="0 0 54 54">
        <path d="m 29.64,32.02 2.7,-4.68 8.16,14.04 h 5.4 C 44.7,39.46 33.3,19.96 32.34,18.04 L 20.58,38.44 16.2,46.06 H 0 C 0.9,44.56 6.96,34 12.78,23.86 16.2,17.92 19.98,11.26 24.24,4 h 5.4 L 18.48,23.32 8.1,41.38 h 5.4 C 20.88,28.36 25.5,20.68 32.34,8.68 32.76,9.4 53.88,46 54,46.06 H 37.8 Z"/>
      </svg>
      <h2 class="ms-2">Session Feedback</h2>
      <div class="ms-auto align-middle">
        <button type="button" class="btn-close" aria-label="Close" @click="goBack()"></button>
      </div>
    </div>

    <ul class="list-group list-group-flush">
      <li class="list-group-item">
        <h2>Rating Summary</h2>
        <SessionRatings :session="session" />
        {{ session.session_type.name }} session at {{ displayVenueTime(session.datetime) }} on {{ displayFullDate(session.datetime) }}
        <span v-if="session.location"> at {{ session.location.name }}</span>
        <span v-if="session.trainer"> led by {{ session.trainer.name }}</span>.
      </li>
      <li class="list-group-item">
        <h2>Feedback Details</h2>
        <table class="table table-hover table-sm">
          <thead>
            <tr>
              <th>Rating</th>
              <th>Comments</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(entry, index) in feedback" :key="index" class="align-middle">
              <td><StarRating :rating="entry.rating" /></td>
              <td>{{ entry.comment }}</td>
            </tr>
          </tbody>
        </table>
      </li>
    </ul>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { displayFullDate, displayVenueTime, getSession, listFeedback, type Feedback, type Session } from '@pfnext/shared'
import SessionRatings from '@/components/SessionRatings.vue'
import StarRating from '@/components/StarRating.vue'
import { useQueryState } from '@/composables/useQueryState'
import { useReturnPath } from '@/composables/useReturnPath'
import { reportApiError, tryApi } from '@/stores/apiError'

const { goBack } = useReturnPath('/sessions.html')
const query = useQueryState()

const session = ref<Session | null>(null)
const feedback = ref<Feedback[]>([])

const sessionId = Number(query.get('id'))
if (sessionId) {
  void tryApi(() => getSession(sessionId)).then((result) => {
    session.value = result ?? null
  })
  void tryApi(() => listFeedback(sessionId)).then((result) => {
    feedback.value = (result ?? []).sort((a, b) => b.rating - a.rating)
  })
} else {
  reportApiError(new Error('missing session id'))
}
</script>
