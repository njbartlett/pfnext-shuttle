const bookings_list = ref([])
const currentMonthStart = startOfMonth(new Date())
const monthly_pagination = ref({
    pages: [-1, 0, 1, 2],
    block: 0,
    month_offset: 0,
    start: currentMonthStart
})
const bookings_pagination = ref({
    past: false
})
const admin_all_user_data = ref([])
const selected_user = ref({})

async function loadBookings() {
    if (!selected_user.value) {
        bookings_list.value = []
        return
    }

    let startDate = new Date()
    startDate.setDate(1)
    startDate.setMonth(startDate.getMonth() + monthly_pagination.value.month_offset)
    startDate.setHours(0, 0, 0, 0)
    
    let endDate = new Date(startDate)
    endDate.setMonth(endDate.getMonth() + 1)
    endDate.setDate(endDate.getDate() - 1)
    endDate.setHours(23, 59, 59, 999)
    
    let params = new URLSearchParams()
    params.append("person_id", selected_user.value.id)
    params.append("from", startDate.toISOString())
    params.append("to", endDate.toISOString())
    httpGetJson("/bookings?" + params.toString(), json => bookings_list.value = json)
}

async function cancelBooking(booking) {
    let params = new URLSearchParams()
    params.append("person_id", booking.person_id)
    params.append("session_id", booking.session_id)
    httpCall("/bookings?" + params.toString(), "DELETE", null, deleted_booking => {
        if (deleted_booking && deleted_booking.person_id === booking.person_id && deleted_booking.session_id === booking.session_id) {
            let index = bookings_list.value.indexOf(booking)
            if (index >= 0) {
                bookings_list.value.splice(index, 1)
            }
        }
    })
}

function renderMonthOffset(offset) {
    var date = new Date(monthly_pagination.value.start)
    date.setMonth(date.getMonth() + offset)
    return date.toLocaleDateString('en-GB', {
        month: 'short',
        year: 'numeric'
    })
}

function resetPagination() {
    monthly_pagination.value.block = 0
    monthly_pagination.value.month_offset = 0
    loadBookings()
}

function scrollPaginationBackwards() {
    monthly_pagination.value.block = monthly_pagination.value.block - monthly_pagination.value.pages.length
}

function scrollPaginationForwards() {
    monthly_pagination.value.block = monthly_pagination.value.block + monthly_pagination.value.pages.length
}

function setMonthPage(offset) {
    monthly_pagination.value.month_offset = offset
    
    loadBookings()
}

function onSelectedMember(event) {
    let name_and_email = event.target.value
    for (var user of admin_all_user_data.value) {
        if (name_and_email === formatNameAndEmail(user.name, user.email)) {
            selected_user.value = user
            loadBookings()
            return
        }
    }
    selected_user.value = null
}

let app = createApp({
    setup() {
        return {
            loggedin, http_err, onLogout,
            bookings_list, cancelBooking,
            monthly_pagination, renderMonthOffset, scrollPaginationBackwards, scrollPaginationForwards, resetPagination, setMonthPage,
            displayDate, displayTime, isPast, isAdmin, encodeLoginReturnUrl,
            admin_all_user_data, selected_user, onSelectedMember, formatNameAndEmail
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

loadAllUsers(users => {
    admin_all_user_data.value = users
    let elem = document.getElementById("selectMemberDataList")
    let current_user = findUserByEmail(loggedin.value.email, admin_all_user_data.value)
    selected_user.value = current_user
    if (elem && current_user) {
        elem.value = formatNameAndEmail(current_user.name, current_user.email)
    }
}).then(loadBookings)

