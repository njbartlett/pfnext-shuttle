<template>
  <div class="container">
    <PageTitle />

    <div v-if="!isAdmin" class="alert alert-danger my-3" role="alert">
      <i class="bi bi-exclamation-triangle-fill"></i>&nbsp;This page is only available to administrators.
    </div>

    <div v-else class="my-3 row g-3">
      <!-- Poll list -->
      <div class="col-lg-7">
        <div class="card">
          <h5 class="card-header d-flex align-items-center">
            <span>All Polls</span>
            <button type="button" class="btn btn-primary btn-sm ms-auto" @click="startNew"><i class="bi bi-plus-circle"></i>&nbsp;New Poll</button>
          </h5>
          <div class="card-body">
            <div v-if="polls.length === 0" class="alert alert-secondary mb-0" role="alert">
              <em>No polls have been created yet.</em>
            </div>
            <div v-else class="table-responsive">
              <table class="table table-hover table-sm align-middle mb-0">
                <thead>
                  <tr>
                    <th scope="col">Question</th>
                    <th scope="col">Status</th>
                    <th scope="col" class="text-end">Votes</th>
                    <th scope="col" class="text-end" title="Maximum votes per member">Limit</th>
                    <th scope="col"></th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="entry in polls" :id="'poll-' + entry.poll.id" :key="entry.poll.id" :class="{ 'table-active': form.id === entry.poll.id }">
                    <td>
                      <a href="#" @click.prevent="edit(entry.poll)">{{ entry.poll.question }}</a>
                      <div v-if="entry.poll.description" class="small text-secondary text-truncate" style="max-width: 24rem;">{{ entry.poll.description }}</div>
                    </td>
                    <td>
                      <span class="badge rounded-pill" :class="entry.poll.open ? 'text-bg-primary' : 'text-bg-secondary'">{{ entry.poll.open ? 'Open' : 'Closed' }}</span>
                    </td>
                    <td class="text-end">{{ entry.votes.length }}</td>
                    <td class="text-end">{{ entry.poll.limit_per_person }}</td>
                    <td class="text-end text-nowrap">
                      <div class="btn-group btn-group-sm" role="group">
                        <button type="button" class="btn btn-outline-primary" title="Edit poll" @click="edit(entry.poll)"><i class="bi bi-pencil"></i></button>
                        <button v-if="entry.poll.open" type="button" class="btn btn-outline-secondary" title="Close poll" @click="setOpen(entry.poll, false)"><i class="bi bi-lock-fill"></i></button>
                        <button v-else type="button" class="btn btn-outline-success" title="Reopen poll" @click="setOpen(entry.poll, true)"><i class="bi bi-unlock-fill"></i></button>
                        <button type="button" class="btn btn-outline-danger" title="Delete poll" @click="askDelete(entry)"><i class="bi bi-trash-fill"></i></button>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
          <div class="card-footer text-secondary small">
            Members can only vote on open polls. Closed polls remain visible with their results. Deleting a poll also deletes all of its votes.
          </div>
        </div>
      </div>

      <!-- Create / edit form -->
      <div class="col-lg-5">
        <div class="card">
          <h5 class="card-header">{{ form.id ? 'Edit Poll' : 'Create New Poll' }}</h5>
          <div class="card-body">
            <form @submit.prevent="save">
              <div class="mb-3">
                <label for="editPollQuestion" class="form-label">Question</label>
                <input id="editPollQuestion" v-model="form.question" type="text" class="form-control" :class="{ 'is-invalid': errors.question }" placeholder="e.g. Which day suits you best for the next social?" required>
                <div class="invalid-feedback">{{ errors.question }}</div>
              </div>
              <div class="mb-3">
                <label for="editPollDescription" class="form-label">Description <span class="text-secondary">(optional)</span></label>
                <textarea id="editPollDescription" v-model="form.description" class="form-control" rows="4" placeholder="Extra detail or instructions shown under the question"></textarea>
              </div>
              <div class="mb-3 row">
                <div class="col-6">
                  <label for="editPollLimit" class="form-label">Votes per Member</label>
                  <input id="editPollLimit" v-model.number="form.limit_per_person" type="number" min="1" step="1" class="form-control" :class="{ 'is-invalid': errors.limit_per_person }" required>
                  <div class="invalid-feedback">{{ errors.limit_per_person }}</div>
                </div>
                <div class="col-6 d-flex align-items-end">
                  <div class="form-check form-switch mb-2">
                    <input id="editPollOpen" v-model="form.open" class="form-check-input" type="checkbox" role="switch">
                    <label for="editPollOpen" class="form-check-label">Open for voting</label>
                  </div>
                </div>
              </div>
            </form>
          </div>
          <div class="d-flex card-footer align-items-center">
            <div class="me-auto p-2">
              <span v-if="outcome" :class="outcome.isError ? 'text-danger' : 'text-success'">{{ outcome.message }}</span>
            </div>
            <div v-if="form.id" class="p-2">
              <button type="button" class="btn btn-outline-secondary" @click="startNew">Cancel</button>
            </div>
            <div class="p-2">
              <button id="editPollSubmit" type="button" class="btn btn-primary" :disabled="!ready" @click="save">{{ form.id ? 'Save' : 'Create' }}</button>
            </div>
          </div>
        </div>
      </div>
    </div>

    <ConfirmModal ref="deleteModal" title="Delete Poll" icon="bi-exclamation-triangle-fill" header-class="text-bg-danger" confirm-label="Delete" confirm-icon="bi-trash" confirm-class="btn-danger" @confirm="confirmDelete">
      <template v-if="deleting">
        <p>Are you sure you want to delete the following poll?</p>
        <ul>
          <li><strong>{{ deleting.poll.question }}</strong></li>
          <li>{{ deleting.poll.open ? 'Open' : 'Closed' }}</li>
          <li>{{ deleting.votes.length }} vote(s) submitted</li>
        </ul>
        <p>This cannot be undone. All votes on this poll will be deleted.</p>
      </template>
    </ConfirmModal>
  </div>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { createPoll, deletePoll, listPolls, updatePoll, type NewPoll, type Poll, type PollWithVotes } from '@pfnext/shared'
import ConfirmModal from '@/components/ConfirmModal.vue'
import PageTitle from '@/components/PageTitle.vue'
import { isAdmin } from '@/stores/auth'
import { tryApi } from '@/stores/apiError'

const polls = ref<PollWithVotes[]>([])
const form = reactive({ id: null as number | null, question: '', description: '', limit_per_person: 1 as number | null, open: true })
const outcome = ref<{ message: string; isError: boolean } | null>(null)
const deleting = ref<PollWithVotes | null>(null)
const deleteModal = ref<InstanceType<typeof ConfirmModal> | null>(null)

const errors = computed(() => {
  // A completely blank question shows no error (the form is just empty), but
  // whitespace-only is called out. Either way the form is not ready.
  const questionBlank = form.question.length === 0
  const questionOk = !questionBlank && form.question.trim().length > 0
  const limit = form.limit_per_person
  const limitOk = limit !== null && Number.isInteger(limit) && limit >= 1
  return {
    question: questionOk || questionBlank ? null : 'Question is required',
    limit_per_person: limitOk ? null : 'Must be a whole number, at least 1',
    ready: questionOk && limitOk
  }
})
const ready = computed(() => errors.value.ready)

async function load() {
  if (!isAdmin.value) {
    return
  }
  // The admin list must always include closed polls, which /polls omits by default
  polls.value = (await tryApi(() => listPolls({ closed: true }))) ?? []
}

function fill(poll: Poll | null) {
  form.id = poll?.id ?? null
  form.question = poll?.question ?? ''
  form.description = poll?.description ?? ''
  form.limit_per_person = poll?.limit_per_person ?? 1
  form.open = poll?.open ?? true
}

function startNew() {
  fill(null)
  outcome.value = null
}

function edit(poll: Poll) {
  fill(poll)
  outcome.value = null
}

function toRequest(poll: { question: string; description: string | null; limit_per_person: number | null; open: boolean }): NewPoll {
  const description = (poll.description ?? '').trim()
  return {
    question: poll.question.trim(),
    description: description.length > 0 ? description : null,
    limit_per_person: poll.limit_per_person ?? 1,
    open: poll.open
  }
}

async function save() {
  if (!ready.value) {
    return
  }
  if (form.id === null) {
    const created = await tryApi(() => createPoll(toRequest(form)))
    if (created) {
      startNew()
      outcome.value = { message: `Created poll "${created.question}"`, isError: false }
      await load()
    }
  } else {
    const saved = await tryApi(() => updatePoll(form.id!, toRequest(form)))
    if (saved) {
      fill(saved)
      outcome.value = { message: 'Saved!', isError: false }
      await load()
    }
  }
}

// Open or close a poll directly from the list, keeping its other fields unchanged
async function setOpen(poll: Poll, open: boolean) {
  const saved = await tryApi(() => updatePoll(poll.id, { ...toRequest(poll), open }))
  if (saved) {
    if (form.id === saved.id) {
      form.open = saved.open
    }
    await load()
  }
}

function askDelete(entry: PollWithVotes) {
  deleting.value = entry
  deleteModal.value?.show()
}

async function confirmDelete() {
  if (!deleting.value) {
    return
  }
  const pollId = deleting.value.poll.id
  const done = await tryApi(async () => {
    await deletePoll(pollId)
    return true
  })
  if (done) {
    if (form.id === pollId) {
      startNew()
    }
    deleting.value = null
    await load()
  }
}

watch(isAdmin, load, { immediate: true })
</script>
