const activity_types = ref([])
const new_adhoc_activity = reactive({
    date: toDateString(new Date()),
    activity_type: -1,
    quantity: 0
})
const new_adhoc_activity_meta = ref({
    step_size: 1,
    units: 'units'
})
const new_adhoc_activity_validation = ref({
    error_msg: null,
    ready: false
})
const all_activities_query = reactive({
    type: -1,
    from: toDateString(startOfMonth(new Date())),
    to: toDateString(new Date())
})
const all_activities_query_validation = ref({
    error_msg: null
})
const all_activities = ref([])
const all_activity_totals = ref({})

function updateNewAdhocActivity(new_data) {
    for (var activity_type of activity_types.value) {
        if (activity_type.id == new_data.activity_type) {
            new_adhoc_activity_meta.value.step_size = activity_type.step_size
            new_adhoc_activity_meta.value.units = activity_type.units
            break
        }
    }

    let error_msg = null
    
    if (!new_data.date) {
        error_msg = "Date is required"
    } else if (new_data.activity_type == null || new_data.activity_type == -1) {
        error_msg = "Activity type must be selected"
    } else if (new_data.quantity == null || new_data.quantity == 0) {
        error_msg = "Quantity must be greater than zero"
    }
    new_adhoc_activity_validation.value = {
        error_msg: error_msg,
        ready: error_msg == null
    }
}

async function loadAllActivities() {
    let params = new URLSearchParams()
    params.append("person_id", loggedin.value.id)
    params.append("from", all_activities_query.from)
    params.append("to", all_activities_query.to)
    if (all_activities_query.type != null && all_activities_query.type != -1) {
        params.append("activity_type", all_activities_query.type)
    }
    httpGetJson("/activities?" + params.toString(), displayAllActivities)
}

function generateDates(from, to) {
    let dates = []

    let currentDate = new Date(to)
    let startDate = new Date(from)
    while (currentDate >= startDate) {
        dates.push(toDateString(currentDate))
        currentDate = new Date(currentDate)
        currentDate.setDate(currentDate.getDate() - 1)
    }
    return dates
}

function displayAllActivities(activities) {
    all_activities.value = activities

    const activityTypes = {}
    for (let activity of activities) {
        let typeGroup = {
            total: 0,
            count: 0,
            max: 0,
            units: activity.activity_type.units,
            dates: {}
        }
        for (let date of generateDates(all_activities_query.from, all_activities_query.to)) {
            let d = new Date(date)

            typeGroup.dates[date] = {
                total: 0,
                count: 0,
                weekend: d.getDay() == 0 || d.getDay() == 6
            }
        }
        activityTypes[activity.activity_type.name] = typeGroup
    }

    for (let activity of activities) {
        summaryForType = activityTypes[activity.activity_type.name]
        summaryForType.count += 1
        summaryForType.total += activity.amount

        dateEntryForType = summaryForType.dates[activity.date]
        dateEntryForType.count += 1
        dateEntryForType.total += activity.amount

        if (dateEntryForType.total > summaryForType.max) {
            summaryForType.max = dateEntryForType.total
        }

        //
    }

    // let dateMap = new Map()
    // for (let date of generateDates(all_activities_query.from, all_activities_query.to)) {
    //     typeGroupForDate = {}
    //     for (activityType of activityTypeMap.keys()) {
    //         typeGroupForDate[activityType] = {
    //             total: 0
    //         }
    //     }
    //     dateMap[date] = typeGroupForDate
    // }

    // for (let activity of activities) {
    //     // Create entry in top-level byType
    //     let typeGroup = activityTypeMap.get(activity.activity_type.name)
    //     typeGroup.total += activity.amount
    //     if (activity.amount > typeGroup.max) {
    //         typeGroup.max = activity.amount
    //     }

    //     // Create type groups under the date for this activity
    //     let dateGroup = dateMap[activity.date]
    //     let typeForDateGroup = dateGroup[activity.activity_type.name]
    //     if (!typeForDateGroup) {
    //         typeForDateGroup = {
    //             total: activity.amount,
    //             units: activity.activity_type.units
    //         }
    //         dateGroup[activity.activity_type.name] = typeForDateGroup
    //     } else {
    //         typeForDateGroup.total += activity.amount
    //     }

    // }

    all_activity_totals.value = {
        byType: activityTypes
    }

}

async function submitAdhocActivity() {
    httpCall('/activities', 'POST', {
        person_id: loggedin.value.id,
        activity_type: new_adhoc_activity.activity_type,
        date: new_adhoc_activity.date,
        amount: new_adhoc_activity.quantity
    }, res => {
        // Clear the new adhoc activity quantity
        new_adhoc_activity.quantity = null
        
        // Load the newly created activity record
        loadAllActivities()
    })
}

async function deleteActivity(activity, index) {
    return fetch(SERVER_URL + "/activities/" + activity.id, {
        method: 'DELETE',
        credentials: 'include'
    }).then(res => {
        if (!res.ok) throw res

        loadAllActivities()
    }).catch(handleHttpError)
}

function toDateString(dt) {
    return String(dt.getFullYear()).padStart(4, '0') + '-' + String(dt.getMonth() + 1).padStart(2, '0') + '-' + String(dt.getDate()).padStart(2, '0')
}

function displayShortDate(date) {
    return date.toLocaleDateString('en-GB', {
        weekday: 'short',
        day: 'numeric',
        month: 'numeric',
        year: '2-digit'
    })
}

function getActivityTypeName(id) {
    if (id == null || id == -1) {
        return "Any"
    }

    for (activity_type of activity_types.value) {
        if (activity_type.id == id) {
            return activity_type.name
        }
    }
    return "Unknown"
}

let app = createApp({
    setup() {
        return {
            // Data
            loggedin, http_err, all_activities_query, all_activities_query_validation, all_activities, all_activity_totals, activity_types, new_adhoc_activity, new_adhoc_activity_meta, new_adhoc_activity_validation,

            // Functions
            submitAdhocActivity, deleteActivity,
            isAdmin, displayDate, displayNumber, displayPercent, formatNameAndEmail, displayShortDate, getActivityTypeName,
            encodeLoginReturnUrl, onLogout
        }
    }
})

watch(new_adhoc_activity, updateNewAdhocActivity)
watch(all_activities_query, new_data => {
    if (!all_activities_query.from  || !all_activities_query.to) {
        all_activities_query_validation.value.error_msg = 'Both from and to dates must be specified'
    } else {
        all_activities_query_validation.value.error_msg = null
        loadAllActivities()
    }
})

app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

httpGetJson("/activity_types", json => {
    activity_types.value = json
})

loadAllActivities()
