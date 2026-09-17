// Members matching the current search (empty until a search has been run)
const user_data = ref([])
// Search criteria entered by the user
const search = reactive({
    name: '',
    email: '',
    role: ''
})
// True once a search has been run and its results are being displayed
const searched = ref(false)
const searching = ref(false)

const selected_user_orig = ref(null)
const selected_user = reactive({
    id: null, name: null, email: null, phone: null, emergency_name: null, emergency_phone: null, medical_info: null, credits: 0, pwd_defined: false, roles: []
})
const edit_user_changed = ref(false)
const new_role = reactive({
    name: null
})
const new_role_validation = ref({
    name: null,
    ready: false
})

// Build the query parameters for the current search, omitting blank criteria.
// Returns null when no criteria have been entered.
function searchParams() {
    let params = new URLSearchParams()
    for (const key of ['name', 'email', 'role']) {
        const value = (search[key] || '').trim()
        if (value.length > 0) {
            params.set(key, value)
        }
    }
    return params.size > 0 ? params : null
}

function hasSearchCriteria() {
    return searchParams() !== null
}

// Run the search. The full member list is never shown: with no criteria the
// results are simply cleared.
function searchMembers() {
    const params = searchParams()
    if (params === null) {
        user_data.value = []
        searched.value = false
        return
    }
    searching.value = true
    httpGetJson("/users/list?" + params.toString(), json => {
        user_data.value = json
        searched.value = true
        searching.value = false

        // Keep the details panel in step with the reloaded data (e.g. after a save)
        if (selected_user.id !== null) {
            const reloaded = json.find(u => u.id === selected_user.id)
            if (reloaded) {
                selected_user_orig.value = reloaded
                validateEditUser(selected_user)
            }
        }
    })
}

function clearSearch() {
    search.name = ''
    search.email = ''
    search.role = ''
    user_data.value = []
    searched.value = false
}

function onSelectedMember(user) {
    selected_user_orig.value = user
    setSelectedMember(user)
}

function setSelectedMember(user) {
    selected_user.id = user ? user.id : null
    selected_user.name = user ? user.name : null
    selected_user.email = user ? user.email : null
    selected_user.phone = user ? user.phone : null
    selected_user.emergency_name = user ? user.emergency_name : null
    selected_user.emergency_phone = user ? user.emergency_phone : null
    selected_user.medical_info = user ? user.medical_info : null
    selected_user.credits = user ? user.credits : 0
    selected_user.pwd_defined = user ? user.pwd_defined : false
    selected_user.roles = user ? user.roles.slice()  : null // clone
}

async function deleteRole(role, user) {
    let i = user.roles.indexOf(role)
    if (i > -1) {
        user.roles.splice(i, 1)
    }
    validateEditUser(user)
}

async function addRole(role, user) {
    let i = user.roles.indexOf(role)
    if (i < 0) {
        user.roles.push(role)
    }
    validateEditUser(user)
    new_role.name = null
}

async function onSave(user) {
    // After saving, refresh the current search results so the list reflects the change
    httpCall("/users/" + user.id, 'PUT', selected_user, searchMembers)
}

function onReset() {
    setSelectedMember(selected_user_orig.value)
}

function validateEditUser(new_data) {
    edit_user_changed.value = !_.isEqual(new_data, selected_user_orig.value)
}

let app = createApp({
    setup() {
        return {
            loggedin, http_err, user_data, search, searched, searching,
            selected_user_orig, selected_user, edit_user_changed,
            new_role, new_role_validation,
            hasSearchCriteria, searchMembers, clearSearch,
            onSelectedMember, addRole, deleteRole, onSave, onReset, onLogout, isAdmin, encodeLoginReturnUrl
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

watch(selected_user, validateEditUser)
watch(new_role, new_data => {
    let val = new_role_validation.value
    if (!new_data.name || new_data.name.length === 0) {
        val.name = null
        val.ready = false
    } else if (new_data.name.match(/^[a-z0-9-]+$/)) {
        val.name = null
        val.ready = true
    } else {
        val.name = "Lowercase letters, digits, or hyphen only"
        val.ready = false
    }
})
