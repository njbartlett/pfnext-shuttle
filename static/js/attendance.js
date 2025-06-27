const sort_params = ref({
    field: 'person_name',
    ascending: true
})
const session = ref(null)
const bookings = ref([])
const adding_user = ref(null)
const waitlist = ref([])
const page_return_path = ref("sessions.html")

// Reference data
const all_user_data = ref({
    count: 0,
    name_email_to_user: null
})

async function loadSession(sessionId) {
    httpGetJson("/sessions/" + sessionId, json => session.value = json)
}

async function loadSessionAttendees(sessionId) {
    httpGetJson("/bookings?session_id=" + sessionId, json => {
        bookings.value = json
        sortByField(bookings.value, sort_params.value.field, sort_params.value.ascending)
    })
}

async function loadSessionWaitlist(sessionId) {
    httpGetJson("/waitlist?session_id=" + sessionId, json => waitlist.value = json)
}

async function loadAllUsersTables() {
    all_user_data.value = {
        count: 0,
        name_email_to_user: null
    }
    loadAllUsers(users => {
        let count = 0
        let name_email_to_user = new Map()
        for (let user of users) {
            let name_email = user.name + ' <' + user.email + '>'
            name_email_to_user.set(name_email, user)
            count++
        }
        all_user_data.value = {
            count: count,
            name_email_to_user: name_email_to_user
        }
    })
}

function addMemberInput(event) {
    let name_email = event.target.value
    let user = all_user_data.value.name_email_to_user.get(name_email)
    adding_user.value = user
}

async function addMember(sessionId, personId) {
    httpCall("/bookings", 'POST', {
        person_id: personId,
        session_id: sessionId
    }, res => {
        document.getElementById("addMemberToSessionInput").value = null
        loadSessionAttendees(sessionId)
    })
}

async function removeMember(sessionId, personId) {
    http_err.value = null
    let params = new URLSearchParams()
    params.append("person_id", personId)
    params.append("session_id", sessionId)
    httpCall("/bookings?" + params.toString(), 'DELETE', null, res => {
        loadSessionAttendees(sessionId)
        loadSessionWaitlist(sessionId)
    })
}

async function toggleAttendance(booking) {
    let new_attended = !booking.attended
    let params = new URLSearchParams()
    params.append("person_id", booking.person_id)
    params.append("session_id", booking.session_id)
    httpCall("/bookings?" + params.toString(), "PUT", {
        attended: new_attended
    }, res => {
        sortByField(bookings.value, sort_params.value.field, sort_params.value.ascending)
    })
}

async function removeWaitlistEntry(waitlist_entry) {
    let params = new URLSearchParams()
    params.append("person_id", waitlist_entry.person_id)
    params.append("session_id", waitlist_entry.session_id)
    httpCall("/waitlist?" + params.toString(), "DELETE", null, res => {
        return loadSessionAttendees(waitlist_entry.session_id).then(_ => loadSessionWaitlist(waitlist_entry.session_id))
    })
}

function applySort(field, ascending = true) {
    sort_params.value = {
        field: field,
        ascending: ascending
    }
    if (bookings.value != null) {
        sortByField(bookings.value, field, ascending)
    }
}

const searchParams = new URLSearchParams(window.location.search)
const sessionId = searchParams.get("id");
if (sessionId) {
    loadSession(sessionId)
    loadSessionAttendees(sessionId)
    loadSessionWaitlist(sessionId)
    loadAllUsersTables()
} else {
    http_err.value = "missing session id"
}

let app = createApp({
    setup() {
        return {
            http_err, loggedin, page_return_path,
            session, bookings, adding_user, all_user_data, waitlist,
            isAdmin, onLogout, displayTime, displayDate, displayDateTime, addMemberInput, addMember, removeMember, toggleAttendance, encodeLoginReturnUrl, goBack,
            removeWaitlistEntry, sort_params, applySort
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

let return_path = searchParams.get("return")
if (return_path) {
    page_return_path.value = decodeURIComponent(return_path)
}