<template>
  <!-- Group goal -->
  <div v-if="challenge.goal">
    <div class="text-primary fs-5">
      Group Goal: {{ displayNumber(challenge.total_all) }} of {{ displayNumber(challenge.goal) }} {{ challenge.activity_type.units }}
    </div>
    <div class="progress-stacked my-2" style="height: 50px">
      <div class="progress" role="progressbar" :style="{ width: displayPercent(challenge.total_for_person / challenge.goal, 0) }">
        <div class="progress-bar progress-bar-striped progress-bar-animated bg-success" style="height: 50px">{{ displayPercent(challenge.total_for_person / challenge.goal, 0) }}</div>
      </div>
      <div class="progress" role="progressbar" :style="{ width: displayPercent((challenge.total_all - challenge.total_for_person) / challenge.goal, 0) }">
        <div class="progress-bar progress-bar-striped progress-bar-animated bg-primary" style="height: 50px">{{ displayPercent(challenge.total_all / challenge.goal, 0) }}</div>
      </div>
    </div>
  </div>

  <!-- Individual goal -->
  <div v-if="challenge.individual_goal">
    <div class="text-success fs-5">
      {{ memberName }}'s Goal: {{ displayNumber(challenge.total_for_person) }} of {{ displayNumber(challenge.individual_goal) }} {{ challenge.activity_type.units }}
    </div>
    <div class="progress my-2" role="progressbar" style="height: 50px">
      <div class="progress-bar progress-bar-striped progress-bar-animated bg-success" :style="{ width: displayPercent(challenge.total_for_person / challenge.individual_goal, 0) }">
        {{ displayPercent(challenge.total_for_person / challenge.individual_goal, 0) }}
      </div>
    </div>
  </div>

  <!-- Daily goal -->
  <div v-if="challenge.daily_goal">
    <div class="text-success fs-1">
      <span class="fs-5 me-1">{{ memberName }}'s Daily Goal: {{ displayNumber(challenge.daily_goal) }}&nbsp;{{ challenge.activity_type.units }}</span>
    </div>
    <div class="text-success">
      <div
        v-for="(daily, index) in challenge.daily_activities"
        :key="daily.date"
        class="badge lh-sm me-1 mb-1"
        :class="daily.completed ? 'text-bg-success' : daily.amount > 0 ? 'text-bg-warning' : 'text-bg-secondary'"
      >
        <span style="font-weight: normal">Day {{ index + 1 }}</span>
        <br>
        {{ daily.amount }}<span v-if="daily.amount > 0">&nbsp;{{ challenge.activity_type.units }}</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// Group, individual and daily progress of one challenge for one member.
// Replaces the challenge_progress_bars Tera include.
import { displayNumber, displayPercent, type ChallengeFull } from '@pfnext/shared'

defineProps<{ challenge: ChallengeFull; memberName: string }>()
</script>
