const MILLIS_IN_WEEK = 1000 * 60 * 60 * 24 * 7
const START_OF_WEEK = 1 // Monday
const PAYMENT_REQUIRED = 402

// Additional user data not contained in the token => used to alert the user if no emergency contact is provided
const user_data = ref(null)

// Main session list
const session_data = ref([])

// Session table page control
const currentWeekStart = startOfWeek(new Date())
const session_pagination = ref({
    pages: [-1, 0, 1, 2],
    block: 0,
    week_offset: 0,
    start: currentWeekStart,
    end: new Date(currentWeekStart.getTime() + MILLIS_IN_WEEK)
})
const view_mode = ref('cal')
const payment_confirm = ref(null)
const waitlist_joined = ref(null)
const selected_session_feedback = ref({
    session: null,
    rating: null,
    comment: null,
    error: null
})

// ADMIN ONLY DATA
const deleting_session = ref({
    id: null,
    datetime: new Date()
})
// END ADMIN ONLY DATA

async function loadSessions() {        
    let pageStart = session_pagination.value.start
    let pageEnd = session_pagination.value.end

    httpGetJson("/sessions?from=" + pageStart.toISOString() + "&to=" + pageEnd.toISOString(), json => {
        session_data.value = json
    })
}

async function loadUser() {
    if (loggedin.value) {
        httpGetJson("/users/" + loggedin.value.id, user => {
            user_data.value = user
        })
    }
}

async function bookSession(session, credits_used) {
    http_err.value = null
    fetch(SERVER_URL + "/bookings", {
        method: "POST",
        body: JSON.stringify({
            person_id: loggedin.value.id,
            session_id: session.id,
            credits_used: credits_used ?? 0
        }),
        credentials: 'include'
    })
    .then(res => {
        if (res.status === PAYMENT_REQUIRED) {
            payment_confirm.value = {
                session: session,
                cost: session.cost
            }
            const confirmPaymentModal = new bootstrap.Modal(document.getElementById('spendCreditsConfirmDialog'))
            confirmPaymentModal.show()
        } else if (!res.ok) {
            throw res
        } else {                
            session.booking_count ++
            session.booked = true
        }
    })
    .catch(error => {
        error.text().then(error_text => {
            http_err.value = error_text
        })
    })
}

async function joinWaitlist(session) {
    const cost = session.cost
    return httpCall("/waitlist", "POST", {
        person_id: loggedin.value.id,
        session_id: session.id
    }, res => res.json()).then(json => {
        waitlist_joined.value = json
        waitlist_joined.value["cost"] = cost

        let modal = new bootstrap.Modal(document.getElementById('joinWaitlistConfirmDialog'))
        modal.show()
        return loadSessions()
    })
}

async function leaveWaitlist(session) {
    let params = new URLSearchParams()
    params.append("person_id", loggedin.value.id)
    params.append("session_id", session.id)
    return httpCall("/waitlist?" + params.toString(), "DELETE", null, res => {
        return loadSessions()
    })
}

async function cancelBooking(session) {
    http_err.value = null
    let params = new URLSearchParams()
    params.append("person_id", loggedin.value.id)
    params.append("session_id", session.id)
    httpCall("/bookings?" + params.toString(), 'DELETE', null, res => {
        session.booking_count --
        session.booked = false
    })
}

function setSessionPage(week_offset) {
    // Update the week
    let currentWeekStart = startOfWeek(new Date())
    session_pagination.value.start = addDays(currentWeekStart, 7 * week_offset)
    session_pagination.value.end = addDays(session_pagination.value.start, 7)

    // Reload data if not already on the current week
    let current = session_pagination.value.week_offset
    session_pagination.value.week_offset = week_offset
    if (current !== week_offset) {
        loadSessions()
    }

    // Update the URL hash string
    let urlParams;
    let hashStr = window.location.hash
    if (hashStr) {
        urlParams = new URLSearchParams(hashStr.substring(1))
    } else {
        urlParams = new URLSearchParams("")
    }
    if (week_offset === 0) {
        urlParams.delete("week")
    } else {            
        let startDate = session_pagination.value.start.toLocaleDateString("sv")
        urlParams.set("week", startDate)
    }
    window.location.hash = urlParams.toString()
}

function isUserTrainerOfSession(session) {
    return isTrainer() && session && session.trainer && session.trainer.id === loggedin.id
}

function onClickDeleteSession(session) {
    deleting_session.value = session
}

async function deleteSession(session) {
    http_err.value = null
    httpCall("/sessions/" + session.id, 'DELETE', null, loadSessions)
}

function scrollPaginationBackwards() {
    session_pagination.value.block = session_pagination.value.block - session_pagination.value.pages.length
}

function scrollPaginationForwards() {
    session_pagination.value.block = session_pagination.value.block + session_pagination.value.pages.length
}

function resetPagination() {
    session_pagination.value.block = 0
    setSessionPage(0)
}

function renderWeekOffset(offset) {
    // if (offset < -1) {
    //     return Math.abs(offset) + " Weeks Ago"
    // } else
    
    if (offset === -1) {
        return "Last Week"
    } else if (offset === 0) {
        return "This Week"
    } else if (offset === 1) {
        return "Next Week"
    }

    let start = new Date(currentWeekStart)
    start.setDate(start.getDate() + offset * 7)
    
    let end = new Date(start)
    end.setDate(end.getDate() + 6)


    return displayDateRange(start, end)
    // } else {
    //     return "+" + offset + " Weeks"
    // }
}

function displayDateCalendar(date, length) {
    let options
    if (length === 'xs') {
        options = {weekday: "narrow"}
    } else if (length === 's') {
        options = {weekday: "short"}
    } else {
        options = {
            weekday: "short",
            day: "numeric",
            month: "short"
        }
    }
    return date.toLocaleDateString(undefined, options)
}

function daysOfWeek() {
    let this_week = session_pagination.value.week_offset === 0
    let days = []
    const today = new Date()
    for (var d = 0; d < 7; d++) {
        let date = new Date(session_pagination.value.start)
        date.setDate(date.getDate() + d)
        let is_today = date.getFullYear() === today.getFullYear() &&
                        date.getMonth() === today.getMonth() &&
                        date.getDate() === today.getDate()
        days.push({
            date: date,
            is_today: is_today,
            is_past: !is_today && date < today,
            index: d
        })
    }
    return days
}

function displayDateRange(start, end) {
    let start_opts = {
        day: 'numeric'
    }
    if (start.getMonth() != end.getMonth()) {
        start_opts.month = 'short'
    }
    if (start.getFullYear() != end.getFullYear()) {
        start_opts.year = 'numeric'
    }
    return start.toLocaleDateString('en-GB', start_opts) + " – " + end.toLocaleDateString('en-GB', {
        day: 'numeric',
        month: 'short',
        year: 'numeric'
    })
}

function sessionsByTime(sessions) {
    const times = ['00:00', '01:00', '02:00', '03:00', '04:00', '05:00', '06:00', '07:00', '08:00', '09:00', '10:00', '11:00', '12:00', '13:00', '14:00', '15:00', '16:00', '17:00', '18:00', '19:00', '20:00', '21:00', '22:00', '23:00']
    const time_map = new Map()
    const today = new Date()
    for (let time of times) {
        let days = []
        for (var d = 0; d < 7; d++) {
            let date = new Date(session_pagination.value.start)
            date.setDate(date.getDate() + d)
            let is_today = date.getFullYear() === today.getFullYear() &&
                            date.getMonth() === today.getMonth() &&
                            date.getDate() === today.getDate()
            days.push({
                is_today: is_today,
                sessions: []
            })
        }
        time_map.set(time, days);
    }

    for (let session of sessions) {
        let session_datetime = new Date(session.datetime)
        let [hour, mins] = displayTime(session_datetime).split(':')
        let session_time_floor_hour = hour + ":00"
        let time_entry = time_map.get(session_time_floor_hour)
        let session_day_of_week = (7 + session_datetime.getDay() - START_OF_WEEK) % 7 // Monday = 0
        if (time_entry && time_entry[session_day_of_week]) {
            time_entry[session_day_of_week].sessions.push(session)
        }
    }

    const result = []
    for (let [time, days] of time_map.entries()) {
        result.push({
            time: time,
            days: days
        })
    }
    return result
}

function afterLogout() {
    user_data.value = null
}

function openSessionFeedback(session) {
    console.log("Opening feedback for session", session)
    selected_session_feedback.value.session = session
    selected_session_feedback.value.rating = session.rating
    selected_session_feedback.value.comment = session.comment
    selected_session_feedback.value.error = null
    feedbackModal.show()
}

function saveSessionFeedback() {
    let params = new URLSearchParams()
    params.append("person_id", loggedin.value.id)
    params.append("session_id", selected_session_feedback.value.session.id)
    let data = {
        feedback: {
            rating: parseInt(selected_session_feedback.value.rating),
            comment: selected_session_feedback.value.comment ?? ""
        }
    }

    let request = {
        method: 'PATCH',
        credentials: 'include',
        headers: {
            'Content-Type': 'application/json'
        },
        body: JSON.stringify(data)
    }
    return fetch(SERVER_URL + '/bookings?' + params.toString(), request).then(res => {
        if (!res.ok) throw res
        feedbackModal.hide()
        return loadSessions()
    }).catch(err => {
        err.text().then(msg => {
            selected_session_feedback.value.error = msg
        })
    })

}

// Enable bootstrap tooltips
const tooltipTriggerList = document.querySelectorAll('[data-bs-toggle="tooltip"]')
const tooltipList = [...tooltipTriggerList].map(tooltipTriggerEl => new bootstrap.Tooltip(tooltipTriggerEl))

let app = createApp({
    setup() {
        return {
            // User and error reporting
            loggedin, http_err, user_data,

            // Session/booking data and callbacks
            session_data, session_pagination, view_mode, payment_confirm,
            bookSession, cancelBooking, setSessionPage,
            daysOfWeek, sessionsByTime,
            joinWaitlist, leaveWaitlist, waitlist_joined, openSessionFeedback, saveSessionFeedback, selected_session_feedback,

            // Misc callbacks and utility functions
            deleting_session, onClickDeleteSession, deleteSession, onLogout, renderWeekOffset, scrollPaginationBackwards, scrollPaginationForwards, resetPagination, displayDate, displayTime, displayDateCalendar, isPast, isAdmin, isTrainer, isUserTrainerOfSession, encodeLoginReturnUrl, renderStarRating, displayPercent
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

loadUser()

let hashStr = window.location.hash;
let urlParams = hashStr ? new URLSearchParams(hashStr.substring(1)) : new URLSearchParams()

// Parse the current week from the URL fragment
let dateStr = urlParams.get("week")
if (dateStr) {
    let targetWeek = startOfWeek(new Date(dateStr))
    let offsetMillis = targetWeek.getTime() - currentWeekStart.getTime()
    session_pagination.value.week_offset = Math.floor(offsetMillis / MILLIS_IN_WEEK)
    session_pagination.value.start = targetWeek
    session_pagination.value.end = new Date(targetWeek.getTime() + MILLIS_IN_WEEK)
    let pagination_block_weeks = session_pagination.value.pages.length

    // Set the pagination block based on the week offset.
    // We want to use Math.floor() to e.g. round 3.5 down to 3.0 but with negative numbers it rounds in the wrong direction (-3.5 becomes -4.0). So we use Math.abs() before floor() and then later reapply the sign.
    session_pagination.value.block = (session_pagination.value.week_offset >= 0 || -1) * Math.floor(Math.abs(session_pagination.value.week_offset) / pagination_block_weeks) * pagination_block_weeks
}

let storage_view_mode = localStorage.getItem("view_mode")
if (storage_view_mode !== null) {
    view_mode.value = storage_view_mode
}
watch(view_mode, (new_value) => {
    localStorage.setItem("view_mode", new_value)
});

loadSessions()

// Create JavaScript Modals
const feedbackModal = new bootstrap.Modal(document.getElementById('feedbackModal'), {
    focus: true,
    keyboard: true
})