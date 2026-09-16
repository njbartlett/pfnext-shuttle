// Bulk session entry: every row becomes one session. Duration is fixed at 60
// minutes and cost is defaulted from the session type; anything needing other
// values goes through edit_session.html instead.

const DEFAULT_DURATION_MINS = 60

let next_row_key = 1
function blankRow() {
    return {
        key: next_row_key++,
        date: null,
        time: null,
        session_type_id: null,
        trainer_id: null,
        location_id: null
    }
}

const rows = ref([blankRow()])
const outcome = ref(null)

// Reference data
const admin_session_type_list = ref([])
const admin_trainer_list = ref([])
const admin_location_list = ref([])
const suggested_session_times = ref([
    "08:00",
    "08:30",
    "09:00",
    "09:30",
    "18:00",
    "18:30",
    "19:00",
    "19:30"
])
const page_return_path = ref("sessions.html")

async function loadAdminData() {
    httpGetJson("/users/list?role=trainer", trainers => admin_trainer_list.value = trainers)
    httpGetJson("/locations", locs => admin_location_list.value = locs)
    httpGetJson("/session_types?deprecated=false", types => admin_session_type_list.value = types)
}

function sessionTypeOf(row) {
    for (var session_type of admin_session_type_list.value) {
        if (session_type.id === row.session_type_id) {
            return session_type
        }
    }
    return null
}

function costText(row) {
    let session_type = sessionTypeOf(row)
    return session_type ? session_type.cost + " cr" : "—"
}

function trainerError(row) {
    let session_type = sessionTypeOf(row)
    if (session_type && session_type.requires_trainer && row.trainer_id === null) {
        return "Sessions of type '" + session_type.name + "' require a trainer to be selected."
    }
    return null
}

// A row shows its missing-field highlighting only once the user has started it
function rowStarted(row) {
    return row.date !== null || row.time !== null || row.session_type_id !== null ||
        row.trainer_id !== null || row.location_id !== null
}

function rowValid(row) {
    return !!row.date && !!row.time && row.session_type_id !== null && trainerError(row) === null
}

const all_valid = computed(() => rows.value.length >= 1 && rows.value.every(rowValid))

function addRow() {
    // Copy the previous row so only what changes needs editing
    let prev = rows.value[rows.value.length - 1]
    let copy = blankRow()
    copy.date = prev.date
    copy.time = prev.time
    copy.session_type_id = prev.session_type_id
    copy.trainer_id = prev.trainer_id
    copy.location_id = prev.location_id
    rows.value.push(copy)
    outcome.value = null
}

function removeRow(index) {
    if (rows.value.length > 1) {
        rows.value.splice(index, 1)
    }
    outcome.value = null
}

async function createAll() {
    let request = rows.value.map(row => {
        let session_type = sessionTypeOf(row)
        return {
            datetime: new Date(row.date + " " + row.time).toISOString(),
            duration_mins: DEFAULT_DURATION_MINS,
            session_type_id: row.session_type_id,
            location_id: row.location_id,
            trainer_id: row.trainer_id,
            cost: session_type ? session_type.cost : 0,
            max_bookings: null,
            notes: null,
            booking_deadline_mins: 0
        }
    })
    httpCall("/sessions/batch", "POST", request, res => {
        return res.json().then(ids => {
            outcome.value = {
                is_error: false,
                message: "Created " + ids.length + " session" + (ids.length === 1 ? "" : "s") + "!"
            }
            rows.value = [blankRow()]
        })
    })
}

loadAdminData()

let app = createApp({
    setup() {
        return {
            loggedin, http_err, outcome, page_return_path,
            rows, admin_trainer_list, admin_session_type_list, admin_location_list, suggested_session_times,
            costText, trainerError, rowStarted, all_valid,
            addRow, removeRow, createAll,
            isAdmin, onLogout, encodeLoginReturnUrl, goBack
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

let return_path = new URLSearchParams(window.location.search).get("return")
if (return_path) {
    page_return_path.value = decodeURIComponent(return_path)
}
