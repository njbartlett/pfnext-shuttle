<template>
  <div class="container">
    <PageTitle />

    <AdminPanel>
      <form class="d-flex align-items-center flex-wrap" @submit.prevent>
        <div>
          <input id="admin_all_members" v-model="allMembers" class="checkbox form-check-input" type="checkbox">&nbsp;
          <label class="form-label" for="admin_all_members">View Votes for All Members</label>
        </div>
        <RouterLink to="/polls_admin.html" class="btn btn-outline-primary btn-sm ms-auto me-4"><i class="bi bi-card-checklist"></i>&nbsp;Manage Polls</RouterLink>
      </form>
    </AdminPanel>

    <div class="my-3">
      <form @submit.prevent>
        <input id="show_closed" v-model="showClosed" class="checkbox form-check-input" type="checkbox">&nbsp;
        <label class="form-label" for="show_closed">Show Closed Polls</label>
      </form>
    </div>

    <div v-if="polls.length === 0" class="alert alert-secondary" role="alert">
      <em v-if="showClosed">No polls available at this time.</em>
      <em v-else>No open polls available at this time.</em>
    </div>

    <div v-for="entry in polls" :id="'poll-' + entry.poll.id" :key="entry.poll.id" class="card mb-3" :class="{ 'border-primary': entry.poll.open }">
      <h5 class="card-header" :class="{ 'text-bg-primary': entry.poll.open }">
        <span v-if="!entry.poll.open">CLOSED:</span> {{ entry.poll.question }}
      </h5>
      <div class="card-body">
        <p v-if="entry.poll.description">{{ entry.poll.description }}</p>
        <table class="table table-sm table-borderless mb-0 vertical-align-middle">
          <thead>
            <tr>
              <th scope="col">Vote</th>
              <th v-if="allMembers" scope="col">Voted By</th>
              <th scope="col"></th>
            </tr>
          </thead>

          <tbody>
            <tr v-for="vote in entry.votes" :key="vote.id">
              <td><strong><i class="bi bi-check2-circle"></i>&nbsp;{{ vote.value }}</strong></td>
              <td v-if="allMembers">{{ vote.person_name }}</td>
              <td>
                <button type="button" :disabled="!entry.poll.open" class="btn btn-outline-danger btn-sm float-end" @click="removeVote(entry, vote)"><i class="bi bi-trash-fill"></i></button>
              </td>
            </tr>
            <tr v-if="entry.votes.length === 0">
              <td><em>No votes submitted yet.</em></td>
            </tr>
          </tbody>

          <!-- Voting, only when viewing one's own votes -->
          <tbody v-if="!allMembers && user">
            <tr v-if="entry.votes.length < entry.poll.limit_per_person">
              <td v-if="entry.poll.open" colspan="2">
                <form @submit.prevent="submit(entry, user.id, entry.newVote)">
                  <div class="input-group">
                    <input v-model="entry.newVote" type="text" class="form-control" :placeholder="'Enter your vote, max ' + entry.poll.limit_per_person + ' vote(s) per member'" required>
                    <button class="btn btn-primary" type="submit" :disabled="!entry.newVote">Submit Vote</button>
                  </div>
                </form>
              </td>
              <td v-else><em>Voting is now closed for this poll.</em></td>
            </tr>
            <tr v-else>
              <td><em>You have reached the maximum number of votes for this poll, thank you!</em></td>
            </tr>
          </tbody>
        </table>
      </div>

      <div v-if="isAdmin" class="card-footer bg-warning-subtle position-relative">
        <div class="position-absolute top-0 end-0 text-warning"><i class="bi bi-lightning-fill" title="Admin Only"></i></div>
        <form class="mb-2" @submit.prevent="adminSubmit(entry)">
          <div class="input-group">
            <MemberPicker v-model="entry.adminVoter" :members="members" placeholder="Vote as member…" />
            <input v-model="entry.adminVote" type="text" class="form-control" :placeholder="'Enter your vote, max ' + entry.poll.limit_per_person + ' vote(s) per member'" required>
            <button class="btn btn-primary" type="submit" :disabled="!entry.adminVote || !entry.adminVoter">Submit Vote</button>
          </div>
        </form>
        <button class="btn btn-outline-primary" type="button" @click="exportCsv(entry)"><i class="bi bi-file-earmark-spreadsheet-fill"></i>&nbsp;Export Poll Data</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { deleteVote, listPolls, submitVote, type PollWithVotes, type UserSummary, type Vote } from '@pfnext/shared'
import AdminPanel from '@/components/AdminPanel.vue'
import MemberPicker from '@/components/MemberPicker.vue'
import PageTitle from '@/components/PageTitle.vue'
import { loadSelectableMembers } from '@/composables/useSelectableMembers'
import { isAdmin, user } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

// A poll plus the per-card form state
interface PollEntry extends PollWithVotes {
  newVote: string
  adminVote: string
  adminVoter: UserSummary | null
}

const polls = ref<PollEntry[]>([])
const showClosed = ref(false)
const allMembers = ref(false)
const members = ref<UserSummary[]>([])

async function load() {
  if (!user.value) {
    polls.value = []
    return
  }
  const result = await tryApi(() =>
    listPolls({ personId: allMembers.value ? undefined : user.value!.id, closed: showClosed.value })
  )
  if (result) {
    polls.value = result.map((entry) => ({ ...entry, newVote: '', adminVote: '', adminVoter: null }))
  }
}

async function submit(entry: PollEntry, personId: number, value: string) {
  entry.newVote = ''
  const vote = await tryApi(() => submitVote(personId, entry.poll.id, value))
  if (vote) {
    entry.votes.push(vote)
  }
}

async function adminSubmit(entry: PollEntry) {
  if (!entry.adminVoter) {
    return
  }
  const voter = entry.adminVoter
  const value = entry.adminVote
  entry.adminVote = ''
  entry.adminVoter = null
  await submit(entry, voter.id, value)
}

async function removeVote(entry: PollEntry, vote: Vote) {
  const done = await tryApi(async () => {
    await deleteVote(vote.id)
    return true
  })
  if (done) {
    entry.votes = entry.votes.filter((v) => v.id !== vote.id)
  }
}

function exportCsv(entry: PollEntry) {
  const rows = ['Value,Voter Name,Voter Email']
  for (const vote of entry.votes) {
    const quoted = `"${vote.value.replace(/"/g, '""')}"`
    rows.push(`${quoted},${vote.person_name},${vote.person_email}`)
  }
  const blob = new Blob([rows.join('\n')], { type: 'text/csv' })
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = `${entry.poll.question.replace(/\s+/g, '_')}_votes.csv`
  anchor.click()
  URL.revokeObjectURL(url)
}

watch([showClosed, allMembers], load)
watch(
  user,
  async (current) => {
    await load()
    members.value = current ? await loadSelectableMembers() : []
  },
  { immediate: true }
)
</script>
