const session_types = reactive([])
const filter = reactive({
    from: null,
    to: null
})
const stats_data = ref([])
const stats_categorized = ref([])

async function loadStats() {
    let urlParams = new URLSearchParams()
    if (filter.from) {
        let from = new Date(filter.from + " 00:00:00Z")
        urlParams.set("from", from.toISOString())
    }
    if (filter.to) {
        let to = new Date(filter.to + " 23:59:59Z")
        urlParams.set("to", to.toISOString())
    }
    for (let session_type of session_types) {
        if (session_type.enabled) {
            urlParams.append("session_type", session_type.id)
        }
    }
    let url = SERVER_URL + "/stats/attendance?" + urlParams.toString()
    httpGetJson("/stats/attendance?" + urlParams.toString(), stats => stats_categorized.value = categorizeStats(stats))
}

function onFilterModified() {
    loadStats()
}

function categorizeStats(raw_data) {
    let categorized = []
    let rank = 0
    let current = null
    for (var entry of raw_data) {
        if (current !== null && current.attended_count !== null && current.attended_count === entry.attended_count) {
            current.subentries.push(entry)
        } else {
            rank ++
            current = {
                rank: rank,
                attended_count: entry.attended_count,
                initial: entry,
                subentries: []
            }
            categorized.push(current)
        }
    }
    return categorized
}

watch(filter, onFilterModified)

const app = createApp({
    setup() {
        return {
            loggedin, http_err, session_types, filter, stats_data, stats_categorized,
            onLogout, isAdmin, encodeLoginReturnUrl
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')


httpGetJson("/session_types", json => {
    for (let session_type of json) {
        session_types.push({
            id: session_type.id,
            name: session_type.name,
            enabled: session_type.requires_trainer
        })
    }
    watch(session_types, onFilterModified)
    loadStats()
})
