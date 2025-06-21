const filter = reactive({
    date: new Date().toISOString().split('T')[0]
})
const log_entries = ref([])

async function loadLogs() {
    let urlParams = new URLSearchParams()
    if (filter.date) {
        let from = new Date(filter.date + " 00:00:00Z")
        let to = new Date(filter.date + " 23:59:59Z")
        urlParams.set("from", from.toISOString())
        urlParams.set("to", to.toISOString())
    }
    let url = SERVER_URL + "/log?" + urlParams.toString()
    httpGetJson("/log?" + urlParams.toString(), json => log_entries.value = json)
}

function prevDate() {
    let datetime = new Date(filter.date)
    let prev = new Date()
    prev.setFullYear(datetime.getFullYear())
    prev.setMonth(datetime.getMonth())
    prev.setDate(datetime.getDate() - 1)
    filter.date = prev.toISOString().split('T')[0]
}

function nextDate() {
    let datetime = new Date(filter.date)
    let prev = new Date()
    prev.setFullYear(datetime.getFullYear())
    prev.setMonth(datetime.getMonth())
    prev.setDate(datetime.getDate() + 1)
    filter.date = prev.toISOString().split('T')[0]
}

watch(filter, loadLogs)

const app = createApp({
    setup() {
        return {
            loggedin, http_err, filter, log_entries,
            onLogout, isAdmin, encodeLoginReturnUrl, displayDateTime, prevDate, nextDate
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')
