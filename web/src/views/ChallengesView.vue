<template>
  <RequireLogin>
    <div class="container">
      <PageTitle />

      <AdminPanel>
        <form @submit.prevent>
          <div class="row">
            <div class="col">
              <label for="selectMemberDataList" class="form-label">View and Log as Member:</label>
              <MemberPicker v-model="selectedMember" input-id="selectMemberDataList" :members="members" />
            </div>
            <div class="col">
              <label for="selectCurrentDate" class="form-label">Set Current Date:</label>
              <input id="selectCurrentDate" v-model="currentDate" type="date" class="form-control">
            </div>
          </div>
        </form>
      </AdminPanel>

      <!-- Tabs -->
      <ul id="challengeTabs" class="nav nav-tabs" role="tablist">
        <li v-for="tab in TABS" :key="tab.id" class="nav-item" role="presentation">
          <button
            :id="tab.id"
            class="nav-link fs-5"
            :class="{ active: tab.id === activeTab }"
            data-bs-toggle="tab"
            :data-bs-target="'#' + tab.id + '-pane'"
            type="button"
            role="tab"
            :aria-controls="tab.id + '-pane'"
            :aria-selected="tab.id === activeTab"
            @click="hash.set('tab', tab.id)"
          >{{ tab.label }}</button>
        </li>
      </ul>

      <div class="tab-content">
        <!-- Current -->
        <div id="current-challenges-tab-pane" class="tab-pane fade" :class="{ 'show active': activeTab === 'current-challenges-tab' }" role="tabpanel" aria-labelledby="current-challenges-tab" tabindex="0">
          <div v-if="currentChallenges.length === 0" class="alert alert-warning my-2 fs-5">
            There are no active challenges at the moment, but you can still <a href="/activities.html">record activities</a>!
          </div>

          <div class="row my-0 gx-2 gy-2">
            <div v-for="challenge in currentChallenges" :key="challenge.id" class="col">
              <div class="card my-2 rounded-3 shadow-sm border-primary">
                <div class="card-header text-bg-primary">
                  <span class="fs-4">{{ challenge.name }}</span>
                  <br>
                  <span>{{ displayShortDate(challenge.start) }} to {{ displayShortDate(challenge.finish) }}</span>
                </div>
                <ul class="list-group list-group-flush">
                  <li v-if="challenge.description" class="list-group-item">
                    <div v-html="challenge.description"></div>
                  </li>

                  <li v-if="challenge.goal || challenge.individual_goal || challenge.daily_goal" class="list-group-item">
                    <ChallengeProgressBars :challenge="challenge" :member-name="selectedMember?.name ?? ''" />
                  </li>

                  <!-- Log activity -->
                  <li v-if="entries[challenge.id]" class="list-group-item">
                    <form @submit.prevent="submitActivity(challenge)">
                      <div class="d-flex flex-wrap align-items-center">
                        <div class="fs-5 text-nowrap me-1">Log Activity:</div>
                        <div class="flex-grow-1 d-flex align-items-center gy-2">
                          <input v-model="entries[challenge.id].date" type="date" class="form-control" :class="{ 'is-invalid': entryErrors(challenge).date }" required>
                          <div class="input-group ms-1 flex-nowrap">
                            <input
                              v-model.number="entries[challenge.id].amount"
                              type="number"
                              min="0"
                              :step="challenge.activity_type.step_size"
                              class="form-control"
                              :class="{ 'is-invalid': entryErrors(challenge).amount }"
                              style="max-width: 160px;"
                              placeholder="Quantity"
                              required
                            >
                            <span class="input-group-text">{{ challenge.activity_type.units }}</span>
                          </div>
                          <div class="ms-auto">
                            <button type="submit" class="btn btn-outline-primary ms-1" :disabled="!entryErrors(challenge).ready">Add</button>
                          </div>
                        </div>
                      </div>
                      <div v-if="entryErrors(challenge).message" class="text-danger"><i class="bi bi-exclamation-circle"></i>&nbsp;{{ entryErrors(challenge).message }}</div>
                    </form>
                  </li>
                </ul>

                <!-- Leaderboard -->
                <div class="card-footer">
                  <span class="fs-5 mt-2">Leaderboard</span>
                  <table class="table table-borderless table-transparent mt-2 align-middle">
                    <tbody v-if="leaderboard(challenge).length === 0">
                      <tr><td class="ps-0 fs-5" colspan="3">No activities recorded yet!</td></tr>
                    </tbody>
                    <tbody v-for="ranking in leaderboard(challenge)" :key="ranking.rank" class="border-bottom">
                      <tr>
                        <td :rowspan="ranking.others.length + 1" class="ps-0 fs-2">
                          <span v-if="ranking.rank === 1" title="Gold Medal">🥇</span>
                          <span v-else-if="ranking.rank === 2" title="Silver Medal">🥈</span>
                          <span v-else-if="ranking.rank === 3" title="Bronze Medal">🥉</span>
                          <span v-else>{{ ranking.rank }}</span>
                        </td>
                        <td class="fs-6 text-nowrap">
                          <span v-if="ranking.first.name" :class="{ 'text-success': isSelected(ranking.first.id) }">{{ ranking.first.name }}</span>
                          <span v-else>&laquo; others &raquo;</span>
                        </td>
                        <td width="100%">
                          <div class="progress" role="progressbar" style="height: 25px">
                            <div class="progress-bar" :class="isSelected(ranking.first.id) ? 'bg-success' : 'bg-primary'" :style="{ width: barWidth(challenge, ranking.score) }">
                              {{ displayNumber(ranking.score) }}&nbsp;{{ ranking.score === 1 ? ranking.first.singular_units : ranking.first.plural_units }}
                            </div>
                          </div>
                        </td>
                      </tr>
                      <tr v-for="entry in ranking.others" :key="entry.id ?? entry.name ?? ''">
                        <td class="fs-6 text-nowrap" :class="{ 'text-success': isSelected(entry.id) }">{{ entry.name }}</td>
                        <td width="100%">
                          <div class="progress" role="progressbar" style="height: 25px">
                            <div class="progress-bar" :class="isSelected(entry.id) ? 'bg-success' : 'bg-primary'" :style="{ width: barWidth(challenge, ranking.score) }">
                              {{ displayNumber(ranking.score) }}&nbsp;{{ ranking.score === 1 ? entry.singular_units : entry.plural_units }}
                            </div>
                          </div>
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Future -->
        <div id="future-challenges-tab-pane" class="tab-pane fade" :class="{ 'show active': activeTab === 'future-challenges-tab' }" role="tabpanel" aria-labelledby="future-challenges-tab" tabindex="0">
          <div v-if="futureChallenges.length === 0" class="fs-5 my-2">There are no upcoming challenges planned yet. Check back later!</div>
          <div v-for="challenge in futureChallenges" :key="challenge.id" class="card rounded-3 shadow-sm my-2">
            <div class="card-header">
              <h5>{{ challenge.name }}</h5>
              {{ displayFullDate(challenge.start) }} to {{ displayFullDate(challenge.finish) }}
            </div>
          </div>
        </div>

        <!-- Past -->
        <div id="past-challenges-tab-pane" class="tab-pane fade" :class="{ 'show active': activeTab === 'past-challenges-tab' }" role="tabpanel" aria-labelledby="past-challenges-tab" tabindex="0">
          <div v-for="challenge in pastChallenges" :key="challenge.id" class="card rounded-3 shadow-sm my-2">
            <div class="card-header">
              <h5>{{ challenge.name }}</h5>
              {{ displayFullDate(challenge.start) }} to {{ displayFullDate(challenge.finish) }}
            </div>
            <div class="card-body">
              <ChallengeProgressBars :challenge="challenge" :member-name="selectedMember?.name ?? ''" />
            </div>
          </div>
        </div>
      </div>
    </div>
  </RequireLogin>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import {
  createActivity, displayFullDate, displayNumber, displayPercent, getChallenge, listChallenges,
  type ChallengeFull, type MemberActivitySummary, type UserSummary
} from '@pfnext/shared'
import AdminPanel from '@/components/AdminPanel.vue'
import ChallengeProgressBars from '@/components/ChallengeProgressBars.vue'
import MemberPicker from '@/components/MemberPicker.vue'
import PageTitle from '@/components/PageTitle.vue'
import RequireLogin from '@/components/RequireLogin.vue'
import { toDateInputValue } from '@/composables/useDates'
import { rankByScore } from '@/composables/useRanking'
import { loadSelectableMembers } from '@/composables/useSelectableMembers'
import { useHashState } from '@/composables/useUrlState'
import { user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

// Days after the finish during which a challenge still counts as current
const GRACE_DAYS = 1
const LEADERBOARD_LIMIT = 10

const TABS = [
  { id: 'current-challenges-tab', label: 'Current' },
  { id: 'future-challenges-tab', label: 'Future' },
  { id: 'past-challenges-tab', label: 'Past' }
]

interface ActivityEntry {
  date: string
  amount: number | null
}

const hash = useHashState()
const activeTab = computed(() => {
  const requested = hash.get('tab')
  return TABS.some((tab) => tab.id === requested) ? requested! : TABS[0].id
})

const members = ref<UserSummary[]>([])
const selectedMember = ref<UserSummary | null>(null)
const currentDate = ref(toDateInputValue(new Date()))
const challenges = ref<ChallengeFull[]>([])
// One "log activity" form per current challenge, keyed by challenge id
const entries = reactive<Record<number, ActivityEntry>>({})

type Currency = 'past' | 'current' | 'future'

function currencyOf(challenge: ChallengeFull): Currency {
  const today = new Date(currentDate.value)
  const finishWithGrace = new Date(challenge.finish)
  finishWithGrace.setDate(finishWithGrace.getDate() + GRACE_DAYS)
  if (finishWithGrace < today) return 'past'
  if (new Date(challenge.start) > today) return 'future'
  return 'current'
}

const pastChallenges = computed(() => challenges.value.filter((c) => currencyOf(c) === 'past'))
const currentChallenges = computed(() => challenges.value.filter((c) => currencyOf(c) === 'current'))
const futureChallenges = computed(() => challenges.value.filter((c) => currencyOf(c) === 'future'))

function leaderboard(challenge: ChallengeFull) {
  return rankByScore(challenge.member_summaries, (summary: MemberActivitySummary) => summary.total_amount)
}

function barWidth(challenge: ChallengeFull, score: number): string {
  const top = challenge.member_summaries[0]?.total_amount ?? 0
  return displayPercent(top > 0 ? score / top : 0, 0)
}

function isSelected(memberId: number | null): boolean {
  return memberId !== null && selectedMember.value?.id === memberId
}

function displayShortDate(date: string): string {
  return new Date(date).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })
}

function entryErrors(challenge: ChallengeFull) {
  const entry = entries[challenge.id]
  let message: string | null = null
  let date = true
  let amount = true
  if (!entry?.date) {
    date = false
    message = 'Date is required'
  } else {
    const chosen = new Date(entry.date)
    if (chosen < new Date(challenge.start) || chosen > new Date(challenge.finish)) {
      date = false
      message = 'Date must be within date range of challenge'
    }
  }
  if (entry && (entry.amount === null || entry.amount < 0)) {
    amount = false
    message = 'Amount must not be negative'
  }
  return { date: !date, amount: !amount, message, ready: message === null && (entry?.amount ?? 0) > 0 }
}

function ensureEntries() {
  for (const challenge of currentChallenges.value) {
    if (!entries[challenge.id]) {
      entries[challenge.id] = { date: toDateInputValue(new Date()), amount: 0 }
    }
  }
}

async function load() {
  if (!selectedMember.value) {
    challenges.value = []
    return
  }
  challenges.value = (await tryApi(() => listChallenges(selectedMember.value!.id, LEADERBOARD_LIMIT, currentDate.value))) ?? []
  ensureEntries()
}

async function submitActivity(challenge: ChallengeFull) {
  const entry = entries[challenge.id]
  if (!entry || !selectedMember.value || !entryErrors(challenge).ready) {
    return
  }
  const memberId = selectedMember.value.id
  const done = await tryApi(async () => {
    await createActivity({
      person_id: memberId,
      challenge_id: challenge.id,
      activity_type: null,
      date: entry.date,
      amount: entry.amount ?? 0
    })
    return true
  })
  if (!done) {
    return
  }
  entry.amount = 0
  // Reload just this challenge for the new progress and leaderboard
  const reloaded = await tryApi(() => getChallenge(challenge.id, memberId, LEADERBOARD_LIMIT, currentDate.value))
  if (reloaded) {
    challenges.value = challenges.value.map((c) => (c.id === reloaded.id ? reloaded : c))
  }
}

watch([selectedMember, currentDate], load)

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
