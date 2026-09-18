<template>
  <RequireLogin>
    <div class="container">
      <PageTitle />

      <div class="my-3 card">
        <h5 class="card-header">Find Members</h5>
        <div class="card-body">
          <form @submit.prevent="search">
            <div class="row g-3 align-items-end">
              <div class="col-md">
                <label for="searchName" class="form-label">Name</label>
                <input id="searchName" v-model="criteria.name" type="search" class="form-control" placeholder="Any part of the name" autocomplete="off">
              </div>
              <div class="col-md">
                <label for="searchEmail" class="form-label">Email Address</label>
                <input id="searchEmail" v-model="criteria.email" type="search" class="form-control" placeholder="Any part of the email address" autocomplete="off">
              </div>
              <div class="col-md">
                <label for="searchRole" class="form-label">Role</label>
                <input id="searchRole" v-model="criteria.role" type="search" class="form-control" placeholder="e.g. member, trainer, admin" autocomplete="off">
              </div>
              <div class="col-md-auto">
                <button type="submit" class="btn btn-primary" :disabled="!hasCriteria || searching"><i class="bi bi-search"></i>&nbsp;Search</button>
                <button type="button" class="btn btn-outline-secondary" @click="clearSearch">Clear</button>
              </div>
            </div>
            <div class="form-text">Enter at least one criterion. Name and email match any part of the text; role must match exactly.</div>
          </form>
        </div>
      </div>

      <div class="my-3 row">
        <div class="col">
          <div class="card">
            <h5 class="card-header d-flex align-items-center">
              <span>Member List</span>
              <span v-if="searched" class="badge rounded-pill text-bg-secondary ms-auto">{{ results.length }} found</span>
            </h5>
            <div class="card-body">
              <div v-if="!searched" class="alert alert-secondary mb-0" role="alert">
                <em>Enter a name, email address and/or role above and click Search to list matching members.</em>
              </div>
              <div v-else-if="results.length === 0" class="alert alert-secondary mb-0" role="alert">
                <em>No members match your search.</em>
              </div>
              <table v-else class="table table-borderless table-hover table-sm">
                <tbody>
                  <tr v-for="member in results" :key="member.id">
                    <td class="text-center" style="width: 2em;"><BibIcon :status="member.status" /></td>
                    <td><a href="#" @click.prevent="select(member)">{{ member.name }}&nbsp;&lt;{{ member.email }}&gt;</a></td>
                    <td>
                      <span v-for="role in member.roles" :key="role" class="badge rounded-pill text-bg-primary" style="margin-right: 1px;">{{ role }}</span>
                    </td>
                    <td :class="member.credits > 0 ? 'text-success' : 'text-secondary'">{{ member.credits }}&nbsp;<i class="bi bi-ticket-perforated"></i></td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>

        <div class="col">
          <div class="card">
            <h5 class="card-header">Member Details</h5>
            <div class="card-body">
              <form @submit.prevent="save">
                <table class="table table-borderless">
                  <tbody>
                    <tr>
                      <td>Name</td>
                      <td><input v-model="draft.name" type="text" class="form-control"></td>
                    </tr>
                    <tr>
                      <td>Email</td>
                      <td><input v-model="draft.email" type="text" class="form-control"></td>
                    </tr>
                    <tr>
                      <td>Phone</td>
                      <td><input v-model="draft.phone" type="text" class="form-control"></td>
                    </tr>
                    <tr>
                      <td>Emergency Contact</td>
                      <td>
                        <div class="row">
                          <div class="col-sm-6"><input v-model="draft.emergency_name" type="text" class="form-control" placeholder="Name"></div>
                          <div class="col-sm-6"><input v-model="draft.emergency_phone" type="text" class="form-control" placeholder="Phone"></div>
                        </div>
                      </td>
                    </tr>
                    <tr>
                      <td>Medical Info</td>
                      <td><textarea v-model="draft.medical_info" class="form-control" rows="5"></textarea></td>
                    </tr>
                    <tr>
                      <td>Status</td>
                      <td>
                        <div class="input-group">
                          <span class="input-group-text" style="width: 2.5em;"><BibIcon :status="draft.status" /></span>
                          <select v-model="draft.status" class="form-select">
                            <option :value="null">None</option>
                            <option value="green">Green Bib</option>
                            <option value="red">Red Bib</option>
                            <option value="blue">Blue Bib</option>
                          </select>
                        </div>
                      </td>
                    </tr>
                    <tr>
                      <td>Credits</td>
                      <td><input v-model.number="draft.credits" type="number" class="form-control"></td>
                    </tr>
                    <tr>
                      <td>Password</td>
                      <td>
                        <span v-if="draft.pwd_defined" class="text-success"><i class="bi bi-check2-circle"></i> Defined</span>
                        <span v-else class="text-danger"><i class="bi bi-x-circle"></i> Undefined</span>
                      </td>
                    </tr>
                    <tr>
                      <td>Roles</td>
                      <td>
                        <span v-if="draft.roles.length === 0" class="badge rounded-pill text-bg-secondary">none</span>
                        <span v-for="role in draft.roles" :key="role">
                          <span class="badge rounded-pill text-bg-primary">{{ role }}&nbsp;<a href="#" class="link-light" title="Remove role" @click.prevent="removeRole(role)"><i class="bi bi-x-circle"></i></a></span>&nbsp;
                        </span>
                        <div class="input-group my-2 p-0">
                          <span class="input-group-text">+</span>
                          <input v-model="newRole" class="form-control" type="text" placeholder="New role name" :class="{ 'is-invalid': newRoleError }">
                          <button type="button" class="btn btn-outline-primary" :disabled="!newRoleReady" @click.prevent="addRole">Add Role</button>
                          <div class="invalid-feedback">{{ newRoleError }}</div>
                        </div>
                      </td>
                    </tr>
                    <tr>
                      <td></td>
                      <td>
                        <button type="submit" class="btn btn-outline-primary" :disabled="!dirty">Save</button>&nbsp;
                        <button type="button" class="btn btn-outline-secondary" :disabled="!dirty" @click.prevent="reset">Reset</button>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </form>
            </div>
          </div>
        </div>
      </div>
    </div>
  </RequireLogin>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { listUsers, updateUser, type UserRecord } from '@pfnext/shared'
import BibIcon from '@/components/BibIcon.vue'
import PageTitle from '@/components/PageTitle.vue'
import RequireLogin from '@/components/RequireLogin.vue'
import { useDirtyTracking } from '@/composables/useDirtyTracking'
import { tryApi } from '@/stores/apiError'

const ROLE_PATTERN = /^[a-z0-9-]+$/

const criteria = reactive({ name: '', email: '', role: '' })
const results = ref<UserRecord[]>([])
const searched = ref(false)
const searching = ref(false)

// The details panel: an editable copy of the selected member
const draft = reactive<UserRecord>(emptyRecord())
let original: UserRecord | null = null
const { dirty, markClean } = useDirtyTracking(draft)
const newRole = ref('')

const hasCriteria = computed(() => Object.values(trimmedCriteria()).some((value) => value !== undefined))

const newRoleError = computed(() =>
  newRole.value.length === 0 || ROLE_PATTERN.test(newRole.value) ? null : 'Lowercase letters, digits, or hyphen only'
)
const newRoleReady = computed(() => newRole.value.length > 0 && newRoleError.value === null)

function emptyRecord(): UserRecord {
  return {
    id: 0, name: '', email: '', phone: null, emergency_name: null, emergency_phone: null,
    medical_info: null, roles: [], credits: 0, pwd_defined: false, status: null
  }
}

function trimmedCriteria() {
  const trim = (value: string) => (value.trim().length > 0 ? value.trim() : undefined)
  return { name: trim(criteria.name), email: trim(criteria.email), role: trim(criteria.role) }
}

// The full member list is never shown: with no criteria the results clear
async function search() {
  if (!hasCriteria.value) {
    results.value = []
    searched.value = false
    return
  }
  searching.value = true
  const found = await tryApi(() => listUsers(trimmedCriteria()))
  searching.value = false
  if (!found) {
    return
  }
  results.value = found
  searched.value = true
  // Keep the details panel in step with the reloaded data (e.g. after a save)
  const reloaded = found.find((member) => member.id === draft.id)
  if (reloaded) {
    select(reloaded)
  }
}

function clearSearch() {
  criteria.name = ''
  criteria.email = ''
  criteria.role = ''
  results.value = []
  searched.value = false
}

function select(member: UserRecord) {
  original = member
  Object.assign(draft, { ...member, roles: [...member.roles], status: member.status ?? null })
  markClean()
}

function reset() {
  if (original) {
    select(original)
  }
}

function removeRole(role: string) {
  draft.roles = draft.roles.filter((r) => r !== role)
}

function addRole() {
  if (newRoleReady.value && !draft.roles.includes(newRole.value)) {
    draft.roles.push(newRole.value)
  }
  newRole.value = ''
}

async function save() {
  if (!draft.id) {
    return
  }
  const done = await tryApi(async () => {
    await updateUser(draft.id, {
      name: draft.name,
      email: draft.email,
      phone: draft.phone,
      emergency_name: draft.emergency_name,
      emergency_phone: draft.emergency_phone,
      medical_info: draft.medical_info,
      roles: draft.roles,
      credits: draft.credits,
      status: draft.status
    })
    return true
  })
  if (done) {
    await search()
  }
}
</script>
