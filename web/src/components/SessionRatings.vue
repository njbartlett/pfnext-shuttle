<template>
  <div v-if="session.avg_rating != null">
    <a data-bs-toggle="collapse" :href="'#' + collapseId" role="button" aria-expanded="false" :aria-controls="collapseId">
      <StarRating :rating="session.avg_rating" />
    </a>
  </div>

  <div :id="collapseId" class="collapse">
    <table class="table table-sm table-transparent table-borderless align-middle">
      <tbody>
        <tr v-for="row in breakdown" :key="row.stars">
          <td class="text-nowrap">{{ row.stars }} <i class="bi bi-star-fill"></i></td>
          <td width="100%">
            <div class="progress" role="progressbar" style="height: 10px">
              <div class="progress-bar bg-secondary" :style="{ width: row.share }"></div>
            </div>
          </td>
          <td>{{ row.count }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
// Average star rating of a session with a collapsible per-star breakdown.
// Replaces the session_ratings Tera include.
import { computed } from 'vue'
import { displayPercent, type Session } from '@pfnext/shared'
import StarRating from './StarRating.vue'

const props = defineProps<{ session: Session }>()

const collapseId = computed(() => 'collapseSessionRatings' + props.session.id)

const breakdown = computed(() => {
  const total = props.session.count_rating_all
  const counts = [
    props.session.count_rating_5,
    props.session.count_rating_4,
    props.session.count_rating_3,
    props.session.count_rating_2,
    props.session.count_rating_1
  ]
  return counts.map((count, index) => ({
    stars: 5 - index,
    count,
    share: total > 0 ? displayPercent(count / total, 0) : '0%'
  }))
})
</script>
