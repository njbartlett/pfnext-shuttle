const sort_params = ref({
    field: 'person_name',
    ascending: true
})
const session = ref(null)
const bookings = ref([])
const page_return_path = ref("sessions.html")

async function loadSession(sessionId) {
    httpGetJson("/sessions/" + sessionId, json => session.value = json)
}

async function loadSessionBookings(sessionId) {
    httpGetJson("/feedback?session_id=" + sessionId, json => {
        bookings.value = json
        sortByField(bookings.value, sort_params.value.field, sort_params.value.ascending)
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
    loadSessionBookings(sessionId)
} else {
    http_err.value = "missing session id"
}

let app = createApp({
    setup() {
        return {
            http_err, loggedin, page_return_path,
            session, bookings,
            isAdmin, onLogout, displayTime, displayDate, displayDateTime, encodeLoginReturnUrl, goBack, renderStarRating, displayPercent,
            sort_params, applySort
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

let return_path = searchParams.get("return")
if (return_path) {
    page_return_path.value = decodeURIComponent(return_path)
}