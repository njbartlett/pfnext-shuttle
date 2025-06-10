const hashStr = window.location.hash
const urlParams = hashStr ? new URLSearchParams(hashStr.substring(1)) : new URLSearchParams()

const filter_data = reactive({
    from: urlParams.has("from") ? urlParams.get("from") : startOfMonth(new Date()),
    to: urlParams.has("to") ? urlParams.get("to") : endOfMonth(new Date()),
    trainer_id: urlParams.has("trainer_id") ? urlParams.get("trainer_id") : null
})
const trainer_list = ref([])
const session_data = ref([])

function loadSessions() {
    var params = new URLSearchParams()
    params.set("attended", true)
    if (filter_data.from) {
        params.set("from", new Date(filter_data.from + "T00:00:00Z").toISOString())
    }
    if (filter_data.to) {
        params.set("to", new Date(filter_data.to + "T23:59:59Z").toISOString())
    }
    if (filter_data.trainer_id) [
        params.set("trainer_id", filter_data.trainer_id)
    ]
    httpGetJson("/sessions?" + params.toString(), json => session_data.value = json)
}

let app = createApp({
    setup() {
        return {
            loggedin, http_err,
            session_data, loadSessions,
            trainer_list,
            filter_data,
            displayDate, displayTime,
            isAdmin, encodeLoginReturnUrl, onLogout
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

loadSessions()
httpGetJson("/users/list?role=trainer", json => trainer_list.value = json)

watch(filter_data, new_data => {
    let urlParams = new URLSearchParams()
    if (new_data.from) {
        urlParams.set("from", new_data.from)
    }
    if (new_data.to) {
        urlParams.set("to", new_data.to)
    }
    if (new_data.trainer_id) {
        urlParams.set("trainer_id", new_data.trainer_id)
    }
    window.location.hash = urlParams.toString()
    loadSessions()
})
