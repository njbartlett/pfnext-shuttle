const challenges = ref([])
const new_activities = reactive([])
const new_activities_validation = ref([])
const all_activities = ref([])

const admin_all_user_data = ref([])
const admin_controls_old = ref(null)
const admin_controls = reactive({
    user: null,
    current_date: dateInTimezone(new Date()),
    leaderboard_limit: 12
})

const GRACE_DAYS = 1

/**
 * Get the date as a yyyy-mm-dd string, corrected for the timezone.
 */
function dateInTimezone(date) {
    const offset = date.getTimezoneOffset()
    date = new Date(date.getTime() - (offset * 60 * 1000))
    return date.toISOString().split('T')[0]
}

function isChallengeStarted(challenge) {
    return new Date(challenge.start) <= new Date(admin_controls.current_date)
}

function isSelectedUserId(id) {
    return admin_controls.user && admin_controls.user.id === id
}

async function loadChallengesAndActivities(user, current_date) {
    loadAllActivities(user)
    loadChallenges(user, current_date)
}

async function loadChallenges(user, current_date) {
    var search_params = new URLSearchParams()
    search_params.append("person_id", user.id)
    search_params.append("date_now", current_date + "T00:00:00Z")
    //search_params.append("leaderboard_limit", admin_controls.leaderboard_limit)
    httpGetJson("/challenges?" + search_params.toString(), displayChallenges)
}

async function reloadChallenge(challenge_index, challenge_id) {
    var params = new URLSearchParams()
    params.append("person_id", admin_controls.user.id)
    //params.append("leaderboard_limit", admin_controls.leaderboard_limit)
    httpGetJson("/challenges/" + challenge_id + "?" + params.toString(), challenge => {
        if (challenge != null) {
            updateNewActivityForChallenge(challenge_index, challenge)
    
            // Splice the activity into the existing challenges list
            let found_index = -1
            for (let index = challenges.value.length - 1; index >= 0; index--) {
                if (challenges.value[index].id == challenge.id) {
                    found_index = index
                    break
                }
            }
            if (found_index >= 0) {
                challenges.value.splice(found_index, 1, challenge)
            }
        }
    })
}

function updateNewActivities(activities) {
    for (let index = 0; index < activities.length; index++) {
        let challenge = challenges.value[index]
        let new_activity = activities[index]
        let val = new_activities_validation.value[index]

        let start = new Date(challenge.start)
        let finish = new Date(challenge.finish)

        if (new_activity && val) {
            val.error_msg = null
            if (new_activity.date) {
                var date = new Date(new_activity.date)
                if (start > date || date > finish) {
                    val.date = 'invalid'
                    val.error_msg = "Date must be within date range of challenge"
                } else {
                    val.date = "valid"
                }
            } else {
                val.date = 'invalid'
                val.error_msg = "Date is required"
            }
            
            if (new_activity.amount !== null && new_activity.amount >= 0) {
                val.amount = "valid"
            } else {
                val.amount = "invalid"
                val.error_msg = "Amount must not be negative"
            }
            val.ready = val.error_msg == null && new_activity.amount > 0
        }
        
    }
}

function updateAdminControls(newValue) {
    for (var user of admin_all_user_data.value) {
        if (newValue && newValue.user_name_email === formatNameAndEmail(user.name, user.email)) {
            newValue.user = user
            break
        }
    }
    if (
        !admin_controls_old.value ||
        (newValue.user && newValue.user.id != admin_controls_old.value.user_id) ||
        newValue.current_date != admin_controls_old.value.current_date
    ) {
        loadChallengesAndActivities(newValue.user, newValue.current_date)
    }
    admin_controls_old.value = {
        user_id: user.id,
        current_date: newValue.current_date
    }
}

async function displayChallenges(json) {
    challenges.value.length = 0
    new_activities.length = 0
    new_activities_validation.value.length = 0

    for (let index = 0; index < json.length; index++) {
        let challenge = json[index]
        updateNewActivityForChallenge(index, challenge)
        challenges.value.push(challenge)
    }
}

function updateNewActivityForChallenge(index, challenge) {
    var now = new Date(admin_controls.current_date)

    var start = new Date(challenge.start)
    var finish = new Date(challenge.finish)
    var finish_with_grace = new Date(finish)
    finish_with_grace.setDate(finish_with_grace.getDate() + GRACE_DAYS)
    
    let new_activity = null
    let validation = null
    if (start <= now && now <= finish_with_grace) {
        new_activity = {
            date: toDateString(now),
            amount: 0,
            units: challenge.activity_type.units,
            step_size: challenge.activity_type.step_size,
        }
        validation = {
            date: 'valid',
            amount: 'valid',
            ready: false,
            error_msg: null
        }
    }
    new_activities.splice(index, 1, new_activity)
    new_activities_validation.value.splice(index, 1, validation)
}

async function loadAllActivities(user) {
    let params = new URLSearchParams()
    params.append("person_id", user.id)
    httpGetJson("/activities?" + params.toString(), json => all_activities.value = json)
}

async function submitActivity(challenge_index, challenge) {
    let new_activity = new_activities[challenge_index]
    let val = new_activities_validation.value[challenge_index]
    if (!new_activity || !val || !val.ready) {
        return
    }
    httpCall("/activities", "POST", {
        person_id: admin_controls.user.id,
        challenge_id: challenge.id,
        date: new_activity.date,
        amount: new_activity.amount
    }, res => {
        // Clear the new_activity field to get ready for next manual entry
        new_activities[challenge_index].amount = 0
    
        // Load the newly created activity record
        loadActivityByLocation(res.headers.get('Location'))
        // Reload the challenge to get the new progress indicators
        reloadChallenge(challenge_index, challenge.id)
    })
}

async function loadActivityByLocation(location) {
    httpGetJson(location, activity => {
        if (activity != null) {
            // Splice the new activity into the user's all_activities table
            all_activities.value.splice(0, 0, activity)
        }
    })
}

async function deleteActivity(activity, index) {
    return fetch(SERVER_URL + "/activities/" + activity.id, {
        method: 'DELETE',
        credentials: 'include'
    }).then(res => {
        if (!res.ok) throw res
        all_activities.value.splice(index, 1)

        // Reload the challenge to get the updated progress bars
        for (let challenge_index = 0; challenge_index < challenges.value.length; challenge_index++) {
            if (challenges.value[challenge_index].id == activity.challenge_id) {
                reloadChallenge(challenge_index, activity.challenge_id)
            }
        }
    }).catch(handleHttpError)
}

function toDateString(dt) {
    return String(dt.getFullYear()).padStart(4, '0') + '-' + String(dt.getMonth() + 1).padStart(2, '0') + '-' + String(dt.getDate()).padStart(2, '0')
}

function calculateMemberBarWidth(member_summaries, index) {
    var percentage = member_summaries[index].total_amount / member_summaries[0].total_amount
    return displayPercent(percentage, 0)
}

let app = createApp({
    setup() {
        return {
            // Data
            loggedin, http_err, admin_all_user_data, challenges, all_activities, new_activities, new_activities_validation, admin_controls,

            // Functions
            submitActivity, deleteActivity,
            isAdmin, displayDate, displayNumber, displayPercent, calculateMemberBarWidth, isChallengeStarted, isSelectedUserId, formatNameAndEmail,
            encodeLoginReturnUrl, onLogout
        }
    }
})

watch(new_activities, updateNewActivities)

app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

loadAllUsers(users => {
    if (users) {
        admin_all_user_data.value = users
        let selected_user = loggedin.value != null ? findUserByEmail(loggedin.value.email, admin_all_user_data.value) : null
        admin_controls.user = selected_user
        updateAdminControls(admin_controls)
    }
    watch(admin_controls, updateAdminControls)
})
