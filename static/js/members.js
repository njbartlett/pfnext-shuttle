// All users
const user_data = ref([])
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

async function loadAllUsersTables() {
    httpGetJson("/users/list", json => user_data.value = json)
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
    httpCall("/users/" + user.id, 'PUT', selected_user, loadAllUsersTables)
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
            loggedin, http_err, user_data, selected_user_orig, selected_user, edit_user_changed,
            new_role, new_role_validation,
            onSelectedMember, addRole, deleteRole, onSave, onReset, onLogout, isAdmin, encodeLoginReturnUrl
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

loadAllUsersTables()

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
