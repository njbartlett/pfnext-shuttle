<template>
  <div class="container">
    <PageTitle />

    <!-- Record ad hoc activities -->
    <div class="card my-2 border-success">
      <h5 class="card-header border-success text-bg-success">Record Ad Hoc Activities</h5>
      <form @submit.prevent="submitAdhoc">
        <div class="card-body">
          <div class="d-flex flex-wrap align-items-center">
            <div class="m-1">
              <label for="newAdhocActivityDateInput">Date:</label>
              <input id="newAdhocActivityDateInput" v-model="adhoc.date" type="date" class="form-control" :class="{ 'is-invalid': adhocError === 'Date is required' }" required>
            </div>
            <div class="m-1 flex-grow-1">
              <label for="newAdhocActivityTypeInput">Activity:</label>
              <select id="newAdhocActivityTypeInput" v-model="adhoc.activityTypeId" class="form-control" :class="{ 'is-invalid': adhocError === 'Activity type must be selected' }" required>
                <option disabled :value="null">Select&hellip;</option>
                <option v-for="activityType in activityTypes" :key="activityType.id" :value="activityType.id">{{ activityType.name }}</option>
              </select>
            </div>
            <div class="m-1">
              <label for="newAdhocActivityQuantityInput">Quantity</label>
              <div class="input-group flex-nowrap">
                <input
                  id="newAdhocActivityQuantityInput"
                  v-model.number="adhoc.quantity"
                  type="number"
                  min="0"
                  :step="adhocType?.step_size ?? 1"
                  class="form-control"
                  :class="{ 'is-invalid': adhocError === 'Quantity must be greater than zero' }"
                  style="max-width: 160px;"
                  placeholder="Quantity"
                  required
                >
                <span class="input-group-text">{{ adhocType?.units ?? 'units' }}</span>
              </div>
            </div>
          </div>
        </div>
      </form>
      <div class="card-footer d-flex align-items-center">
        <span><strong>NB</strong>: this activity will not be counted towards a <RouterLink to="/challenges.html">Challenge</RouterLink>.</span>
        <button type="button" class="btn btn-outline-primary ms-auto" :disabled="adhocError !== null" @click="submitAdhoc">Add</button>
      </div>
    </div>

    <!-- Query -->
    <div class="card border-primary">
      <h5 class="card-header border-primary d-flex align-items-center p-0">
        <button type="button" class="btn btn-sm" data-bs-toggle="collapse" data-bs-target="#queryCollapse" aria-expanded="false" aria-controls="queryCollapse">
          <i class="bi bi-caret-right-fill text-collapsed"></i>
          <i class="bi bi-caret-down-fill text-expanded"></i>
        </button>
        Query Activities
      </h5>
      <div id="queryCollapse" class="card-body collapse">
        <form @submit.prevent>
          <div class="flex-grow-1 d-flex flex-wrap align-items-center">
            <div class="flex-grow-1 d-flex flex-nowrap align-items-center">
              <label for="activityQueryDateFrom">From:</label>
              <input id="activityQueryDateFrom" v-model="query.from" type="date" class="form-control flex-grow-1 m-1" required>
            </div>
            <div class="flex-grow-1 d-flex flex-nowrap align-items-center">
              <label for="activityQueryDateTo">To:</label>
              <input id="activityQueryDateTo" v-model="query.to" type="date" class="form-control flex-grow-1 m-1">
            </div>
            <div class="flex-grow-1 d-flex flex-nowrap align-items-center">
              <label for="activityQueryType">Type:</label>
              <select id="activityQueryType" v-model="query.activityTypeId" class="form-control flex-grow-1 m-1">
                <option :value="null">Any</option>
                <option v-for="activityType in activityTypes" :key="activityType.id" :value="activityType.id">{{ activityType.name }}</option>
              </select>
            </div>
          </div>
          <div v-if="queryError" class="text-danger"><i class="bi bi-exclamation-circle"></i>&nbsp;{{ queryError }}</div>
        </form>
      </div>
    </div>

    <!-- Daily totals per activity type -->
    <div class="row my-0 gx-2 gy-2">
      <div v-for="group in totalsByType" :key="group.name" class="col-sm">
        <div class="card border-primary">
          <h5 class="card-header border-primary text-bg-primary">{{ group.name }}</h5>
          <div class="card-body">
            <table class="table table-sm table-transparent align-middle">
              <tbody>
                <tr v-for="day in group.days" :key="day.date" :class="{ 'table-active': day.weekend }">
                  <td class="text-nowrap">{{ displayShortDate(day.date) }}</td>
                  <td width="100%" class="p-0">
                    <div class="progress progress-squared bg-transparent" role="progressbar" style="height: 20px">
                      <div class="progress-bar progress-bar-striped bg-info text-bg-info" :style="{ width: displayPercent(group.max > 0 ? day.total / group.max : 0, 0) }">
                        <span v-if="day.total">{{ displayNumber(day.total) }}&nbsp;{{ group.units }}</span>
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

    <!-- All activities -->
    <div class="card my-2 border-primary">
      <h5 class="card-header border-primary text-bg-primary">All Activities</h5>
      <div class="card-body">
        <table v-if="activities.length > 0" class="table table-borderless table-striped table-sm align-middle">
          <thead>
            <tr>
              <th scope="col">Date</th>
              <th scope="col">Challenge</th>
              <th scope="col">Activity</th>
              <th scope="col">Quantity</th>
              <th scope="col"></th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="activity in activities" :key="activity.id">
              <td>{{ displayShortDate(activity.date) }}</td>
              <td>
                <span v-if="activity.challenge">{{ activity.challenge.name }}</span>
                <span v-else>N/A</span>
              </td>
              <td>{{ activity.activity_type.name }}</td>
              <td>{{ displayNumber(activity.amount) }}&nbsp;{{ activity.activity_type.units }}</td>
              <td><button class="btn btn-sm btn-outline-danger float-end" title="Delete activity" @click="remove(activity)"><i class="bi bi-trash"></i></button></td>
            </tr>
          </tbody>
        </table>
        <div v-else>
          There are no activities of type "{{ queryTypeName }}" to show for the period {{ displayFullDate(query.from) }} to {{ displayFullDate(query.to) }}.
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import {
  createActivity, deleteActivity, displayFullDate, displayNumber, displayPercent, listActivities, listActivityTypes,
  startOfMonth, type Activity, type ActivityType
} from '@pfnext/shared'
import PageTitle from '@/components/PageTitle.vue'
import { toDateInputValue } from '@/composables/useDates'
import { user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const activityTypes = ref<ActivityType[]>([])
const activities = ref<Activity[]>([])

const adhoc = reactive({
  date: toDateInputValue(new Date()),
  activityTypeId: null as number | null,
  quantity: 0 as number | null
})

const query = reactive({
  activityTypeId: null as number | null,
  from: toDateInputValue(startOfMonth(new Date())),
  to: toDateInputValue(new Date())
})

const adhocType = computed(() => activityTypes.value.find((t) => t.id === adhoc.activityTypeId) ?? null)

const adhocError = computed(() => {
  if (!adhoc.date) return 'Date is required'
  if (adhoc.activityTypeId === null) return 'Activity type must be selected'
  if (!adhoc.quantity) return 'Quantity must be greater than zero'
  return null
})

const queryError = computed(() => (query.from && query.to ? null : 'Both from and to dates must be specified'))

const queryTypeName = computed(() =>
  query.activityTypeId === null ? 'Any' : (activityTypes.value.find((t) => t.id === query.activityTypeId)?.name ?? 'Unknown')
)

interface DayTotal {
  date: string
  total: number
  weekend: boolean
}

interface TypeTotals {
  name: string
  units: string
  max: number
  days: DayTotal[]
}

// Every day of the queried range, latest first
function daysInRange(from: string, to: string): string[] {
  const days: string[] = []
  const start = new Date(from)
  for (let current = new Date(to); current >= start; current.setDate(current.getDate() - 1)) {
    days.push(toDateInputValue(current))
  }
  return days
}

// One bar chart per activity type: daily totals across the queried range
const totalsByType = computed<TypeTotals[]>(() => {
  const groups = new Map<string, TypeTotals & { byDate: Map<string, DayTotal> }>()
  const days = daysInRange(query.from, query.to)
  for (const activity of activities.value) {
    const name = activity.activity_type.name
    let group = groups.get(name)
    if (!group) {
      const dayTotals = days.map((date) => {
        const weekday = new Date(date).getDay()
        return { date, total: 0, weekend: weekday === 0 || weekday === 6 }
      })
      group = {
        name,
        units: activity.activity_type.units,
        max: 0,
        days: dayTotals,
        byDate: new Map(dayTotals.map((day) => [day.date, day]))
      }
      groups.set(name, group)
    }
    const day = group.byDate.get(activity.date)
    if (day) {
      day.total += activity.amount
      group.max = Math.max(group.max, day.total)
    }
  }
  return [...groups.values()].map(({ byDate: _byDate, ...group }) => group)
})

function displayShortDate(date: string): string {
  return new Date(date).toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'numeric', year: '2-digit' })
}

async function load() {
  if (!user.value || queryError.value) {
    return
  }
  activities.value =
    (await tryApi(() =>
      listActivities({
        personId: user.value!.id,
        from: query.from,
        to: query.to,
        activityTypeId: query.activityTypeId ?? undefined
      })
    )) ?? []
}

async function submitAdhoc() {
  if (adhocError.value || !user.value) {
    return
  }
  const done = await tryApi(async () => {
    await createActivity({
      person_id: user.value!.id,
      challenge_id: null,
      activity_type: adhoc.activityTypeId,
      date: adhoc.date,
      amount: adhoc.quantity ?? 0
    })
    return true
  })
  if (done) {
    adhoc.quantity = null
    await load()
  }
}

async function remove(activity: Activity) {
  const done = await tryApi(async () => {
    await deleteActivity(activity.id)
    return true
  })
  if (done) {
    await load()
  }
}

watch(query, load)
watch(
  user,
  async (current) => {
    activities.value = []
    if (!current) {
      return
    }
    activityTypes.value = (await tryApi(listActivityTypes)) ?? []
    await load()
  },
  { immediate: true }
)
</script>
