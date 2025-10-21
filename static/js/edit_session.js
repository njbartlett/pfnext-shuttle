const session_data = reactive({
    id: null,
    date: null,
    time: null,
    duration_mins: null,
    session_type_id: null,
    trainer_id: null,
    location_id: null,
    cost: null,
    max_bookings_enabled: false,
    max_bookings: null,
    notes: null,
    booking_deadline_hours: 0,
    booking_deadline_mins: 0
})
const session_data_validation = ref({
    id: null,
    date: null,
    time: null,
    duration_mins: null,
    session_type_id: null,
    trainer_id: null,
    location_id: null,
    cost: null,
    max_bookings: null,
    notes: null,
    ready: false
})

// Reference data
const outcome = ref({
    is_error: false,
    message: ''
})
const admin_user_data = ref(null)
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
    admin_user_data.value = {
        count: 0,
        name_email_to_user: null
    }
    // Fetch all users and load into the admin_user_data map
    httpGetJson("/users/list", users => {
        let count = 0
        let name_email_to_user = new Map()
        for (let user of users) {
            let name_email = user.name + ' <' + user.email + '>'
            name_email_to_user.set(name_email, user)
            count++
        }
        admin_user_data.value = {
            count: count,
            name_email_to_user: name_email_to_user
        }
    })

    // Fetch list of trainers
    httpGetJson("/users/list?role=trainer", trainers => admin_trainer_list.value = trainers)
    httpGetJson("/locations", locs => admin_location_list.value = locs);
    httpGetJson("/session_types?deprecated=false", types => admin_session_type_list.value = types)
}

async function loadSession() {
    let hashStr = window.location.hash
    let urlParams = hashStr ? new URLSearchParams(hashStr.substring(1)) : new URLSearchParams()
    
    const editSessionId = urlParams.get("edit")
    const copySessionId = urlParams.get("copy")

    if (editSessionId) {
        // Get data from an existing session including its ID
        httpGetJson("/sessions/" + editSessionId, session => {
            session_data.id = session.id
            copySessionFields(session, session_data)
        })
    } else if (copySessionId) {
        // Get data from an existing session but leave our session ID blank
        // to force creation instead of edit.
        httpGetJson("/sessions/" + copySessionId, session => {
            copySessionFields(session, session_data)
        })
    }
}

function copySessionFields(from_session, to_session) {
    let dt = new Date(from_session.datetime)
    to_session.date = String(dt.getFullYear()).padStart(4, '0') + '-' + String(dt.getMonth() + 1).padStart(2, '0') + '-' + String(dt.getDate()).padStart(2, '0')
    to_session.time = String(dt.getHours()).padStart(2, '0') + ':' + String(dt.getMinutes()).padStart(2, '0')
    to_session.duration_mins = from_session.duration_mins
    to_session.session_type_id = from_session.session_type.id
    to_session.trainer_id = from_session.trainer ? from_session.trainer.id : null
    to_session.location_id = from_session.location ? from_session.location.id : null
    to_session.cost = from_session.cost
    to_session.max_bookings_enabled = from_session.max_booking_count !== null
    to_session.max_bookings = from_session.max_booking_count
    to_session.notes = from_session.notes
    to_session.booking_deadline_hours = from_session.booking_deadline_duration_mins !== null ? Math.floor(from_session.booking_deadline_duration_mins / 60) : 0
    to_session.booking_deadline_mins = from_session.booking_deadline_duration_mins !== null ? from_session.booking_deadline_duration_mins % 60 : 0
}

async function saveSession(session, createAnother) {
    let datetime = new Date(session.date + " " + session.time)
    let request = {
        datetime: datetime.toISOString(),
        duration_mins: session.duration_mins,
        session_type_id: session.session_type_id,
        location_id: session.location_id,
        trainer_id: session.trainer_id,
        cost: Number(session.cost),
        max_bookings: session.max_bookings_enabled ? session.max_bookings : null,
        notes: session.notes,
        booking_deadline_mins: Number(session.booking_deadline_hours) * 60 + Number(session.booking_deadline_mins)
    }
    let url = session.id ? "/sessions/" + session.id : "/sessions"
    httpCall(url, session.id ? "PUT" : "POST", request, res => {
        if (!res.ok) throw res
        if (res.status === 201) {
            // Created new session, load it in edit mode
            res.text().then(text => {
                let urlParams = new URLSearchParams()
                urlParams.set(createAnother ? "copy" : "edit", parseInt(text))
                window.location.hash = urlParams.toString()
                loadSession()
            })
        } else if (createAnother) {
            let urlParams = new URLSearchParams()
            urlParams.set("copy", session.id)
            window.location.hash = urlParams.toString()
            loadSession()
        } else {
            outcome.value = {
                message: "Saved!",
                is_error: false
            }
        }
    })
}

function validateEditSession(new_data, val) {
    val.date = new_data.date ? null : "Date is required"
    val.time = new_data.time ? null : "Time is required"

    val.trainer_id = null
    if (new_data.session_type_id) {
        val.session_type_id = null
        if (new_data.trainer_id === null) {
            for (var session_type of admin_session_type_list.value) {
                if (session_type.id === new_data.session_type_id && session_type.requires_trainer) {
                    val.trainer_id = "Sessions of type '" + session_type.name + "' require a trainer to be selected."
                }
            }
        }
    } else {
        val.session_type_id = "Session type must be selected"
    }
    val.duration_mins = new_data.duration_mins !== null && new_data.duration_mins >= 0 ? null : "Duration must be positive"
    // val.trainer_id = new_data.trainer_id ? null : "Trainer must be selected"
    // val.location_id = new_data.location_id ? null : "Location must be selected"
    val.max_bookings = !new_data.max_bookings_enabled || isNonNegative(new_data.max_bookings) ? null : "Max bookings must be a non-negative number if enabled"
    val.cost = isNonNegative(new_data.cost) ? null : "Cost must be a non-negative number" 

    val.ready = val.date === null &&
        val.time === null &&
        val.session_type_id === null &&
        val.duration_mins === null &&
        val.trainer_id === null &&
        val.location_id === null &&
        val.cost === null &&
        val.max_bookings === null
}

function isNonNegative(input) {
    return input !== null && input !== '' && Number(input) >= 0
}

function onChangeSessionType() {
    let session_type_id = session_data.session_type_id
    for (var session_type of admin_session_type_list.value) {
        if (session_type.id === session_type_id) {
            session_data.cost = session_type.cost
            return
        }
    }
    session_data.cost = 0
}

function calculateDeadline(date, time, hours, mins) {
    let deadline = new Date(date + " " + time)
    deadline.setHours(deadline.getHours() - hours)
    deadline.setMinutes(deadline.getMinutes() - mins)
    return deadline.toLocaleDateString('en-GB', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit'
    })
}

watch(session_data, new_data => validateEditSession(new_data, session_data_validation.value))

loadAdminData().then(loadSession)

let app = createApp({
    setup() {
        return {
            loggedin, http_err, outcome, page_return_path,
            session_data, session_data_validation, onChangeSessionType,
            admin_user_data, admin_trainer_list, admin_session_type_list, admin_location_list, suggested_session_times,
            saveSession, isAdmin, onLogout, encodeLoginReturnUrl, goBack, calculateDeadline
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

let return_path = new URLSearchParams(window.location.search).get("return")
if (return_path) {
    page_return_path.value = decodeURIComponent(return_path)
}