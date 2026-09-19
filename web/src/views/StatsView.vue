<template>
  <div class="container">
    <PageTitle />

    <div class="row">
      <div class="col">
        <div class="card my-2">
          <h6 class="card-header">Date Range</h6>
          <div class="card-body">
            <form @submit.prevent>
              <div class="row">
                <div class="col">
                  <label for="fromDate" class="form-label">From</label>
                  <input id="fromDate" v-model="filter.from" class="form-control" type="date">
                </div>
                <div class="col">
                  <label for="toDate" class="form-label">To</label>
                  <input id="toDate" v-model="filter.to" class="form-control" type="date">
                </div>
              </div>
            </form>
          </div>
        </div>
      </div>
      <div class="col">
        <div class="card my-2">
          <h6 class="card-header">Included Session Types</h6>
          <div class="card-body">
            <div v-for="sessionType in sessionTypes" :key="sessionType.id" class="form-check">
              <input :id="'sessionType' + sessionType.id" v-model="sessionType.enabled" type="checkbox" class="form-check-input">
              <label class="form-check-label" :for="'sessionType' + sessionType.id">{{ sessionType.name }}</label>
            </div>
          </div>
        </div>
      </div>
    </div>

    <table class="table align-middle">
      <thead>
        <tr>
          <th scope="col">Rank</th>
          <th scope="col">Sessions Attended</th>
          <th scope="col">Name</th>
        </tr>
      </thead>
      <tbody v-for="ranking in rankings" :key="ranking.rank">
        <tr>
          <td :rowspan="ranking.others.length + 1">
            <span v-if="ranking.rank === 1" title="Gold Medal" class="fs-1">🥇</span>
            <span v-else-if="ranking.rank === 2" title="Silver Medal" class="fs-1">🥈</span>
            <span v-else-if="ranking.rank === 3" title="Bronze Medal" class="fs-1">🥉</span>
            <span v-else>{{ ranking.rank }}</span>
          </td>
          <td :rowspan="ranking.others.length + 1">{{ ranking.score }}</td>
          <td>{{ ranking.first.name }}</td>
        </tr>
        <tr v-for="entry in ranking.others" :key="entry.person_id">
          <td>{{ entry.name }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { getAttendanceStats, listSessionTypes, type AttendanceStat } from '@pfnext/shared'
import PageTitle from '@/components/PageTitle.vue'
import { utcDayEnd, utcDayStart } from '@/composables/useDates'
import { rankByScore } from '@/composables/useRanking'
import { user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

interface SelectableType {
  id: number
  name: string
  enabled: boolean
}

const filter = reactive({ from: '', to: '' })
const sessionTypes = ref<SelectableType[]>([])
const stats = ref<AttendanceStat[]>([])

// Members with equal attendance share a rank
const rankings = computed(() => rankByScore(stats.value, (stat) => stat.attended_count))

async function load() {
  if (!user.value) {
    stats.value = []
    return
  }
  stats.value =
    (await tryApi(() =>
      getAttendanceStats({
        from: filter.from ? utcDayStart(filter.from) : undefined,
        to: filter.to ? utcDayEnd(filter.to) : undefined,
        sessionTypeIds: sessionTypes.value.filter((t) => t.enabled).map((t) => t.id)
      })
    )) ?? []
}

watch([filter, sessionTypes], load, { deep: true })

watch(
  user,
  async (current) => {
    if (!current) {
      stats.value = []
      return
    }
    const types = await tryApi(() => listSessionTypes())
    // Trainer-led types are the ones that count by default
    sessionTypes.value = (types ?? []).map((t) => ({ id: t.id, name: t.name, enabled: t.requires_trainer }))
  },
  { immediate: true }
)
</script>
