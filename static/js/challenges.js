const GRACE_DAYS = 1
const DEFAULT_LEADERBOARD_LIMIT = 3

const past_challenges = ref([])
const current_challenges = ref([])
const future_challenges = ref([])

const new_activities = reactive([])
const new_activities_validation = ref([])

const admin_all_user_data = ref([])
const admin_controls_old = ref(null)
const admin_controls = reactive({
    user: null,
    current_date: dateInTimezone(new Date()),
    leaderboard_limit: DEFAULT_LEADERBOARD_LIMIT
})

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

async function loadChallenges(user, current_date) {
    var search_params = new URLSearchParams()
    search_params.append("person_id", user.id)
    search_params.append("date_now", current_date + "T00:00:00Z")
    //search_params.append("leaderboard_limit", admin_controls.leaderboard_limit)
    httpGetJson("/challenges?" + search_params.toString(), displayChallenges)
}

const PAST_CHALLENGE = -1
const CURRENT_CHALLENGE = 0
const FUTURE_CHALLENGE = 1

function getChallengeCurrency(challenge) {
    const now = new Date(admin_controls.current_date)
    var start = new Date(challenge.start)
    var finish = new Date(challenge.finish)
    var finish_with_grace = new Date(finish)
    finish_with_grace.setDate(finish_with_grace.getDate() + GRACE_DAYS)
    
    if (finish_with_grace < now) {
        return PAST_CHALLENGE
    } else if (start > now) {
        return FUTURE_CHALLENGE
    } else {
        return CURRENT_CHALLENGE
    }
}

async function reloadChallenge(challenge_index, challenge_id) {
    var params = new URLSearchParams()
    params.append("person_id", admin_controls.user.id)
    params.append("leaderboard_limit", admin_controls.leaderboard_limit)
    httpGetJson("/challenges/" + challenge_id + "?" + params.toString(), challenge => {
        if (challenge != null) {
            switch (getChallengeCurrency(challenge)) {
                case PAST_CHALLENGE:
                    spliceChallenge(challenge, past_challenges.value);
                    break;
                case CURRENT_CHALLENGE:
                    updateNewActivityForChallenge(challenge_index, challenge);
                    spliceChallenge(challenge, current_challenges.value);
                    break;
                case FUTURE_CHALLENGE:
                    spliceChallenge(challenge, future_challenges.value);
                    break;
            }
        }
    })
}

// Splice a challenge into an existing challenges list
function spliceChallenge(challenge, challenges) {
    let found_index = -1
    for (let index = challenges.length - 1; index >= 0; index--) {
        if (challenges[index].id == challenge.id) {
            found_index = index
            break
        }
    }
    if (found_index >= 0) {
        challenges.splice(found_index, 1, challenge)
    }
}

function updateNewActivities(activities) {
    for (let index = 0; index < activities.length; index++) {
        let challenge = current_challenges.value[index]
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
        loadChallenges(newValue.user, newValue.current_date)
    }
    admin_controls_old.value = {
        user_id: user.id,
        current_date: newValue.current_date
    }
}

async function displayChallenges(json) {
    past_challenges.value.length = 0
    current_challenges.value.length = 0
    future_challenges.value.length = 0
    new_activities.length = 0
    new_activities_validation.value.length = 0

    for (let index = 0; index < json.length; index++) {
        let challenge = json[index]
        switch (getChallengeCurrency(challenge)) {
            case PAST_CHALLENGE:
                past_challenges.value.push(challenge)
                break;
            case CURRENT_CHALLENGE:
                updateNewActivityForChallenge(index, challenge)
                current_challenges.value.push(challenge)
                break;
            case FUTURE_CHALLENGE:
                future_challenges.value.push(challenge)
                break;
        }
    }
}

function updateNewActivityForChallenge(index, challenge) {
    let new_activity = {
        date: toDateString(new Date()),
        amount: 0,
        units: challenge.activity_type.units,
        step_size: challenge.activity_type.step_size,
    }
    let validation = {
        date: 'valid',
        amount: 'valid',
        ready: false,
        error_msg: null
    }
    new_activities.splice(index, 1, new_activity)
    new_activities_validation.value.splice(index, 1, validation)
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
    
        // Reload the challenge to get the new progress indicators
        reloadChallenge(challenge_index, challenge.id)
    })
}


function toDateString(dt) {
    return String(dt.getFullYear()).padStart(4, '0') + '-' + String(dt.getMonth() + 1).padStart(2, '0') + '-' + String(dt.getDate()).padStart(2, '0')
}

function displayShortDate(date) {
    return new Date(date).toLocaleDateString('en-GB', {
        day: 'numeric',
        month: 'short',
        year: 'numeric'
    })
}


function calculateMemberBarWidth(member_summaries, index) {
    var percentage = member_summaries[index].total_amount / member_summaries[0].total_amount
    return displayPercent(percentage, 0)
}

function tabSelected(e) {
    let targetTabId = e.delegateTarget.id
    const params = new URLSearchParams()
    params.append("tab", targetTabId)
    history.replaceState("", "", '#' + params.toString())
}


let app = createApp({
    setup() {
        return {
            // Data
            loggedin, http_err, admin_all_user_data, past_challenges, current_challenges, future_challenges, new_activities, new_activities_validation, admin_controls,

            // Functions
            submitActivity, tabSelected,
            isAdmin, displayDate, displayShortDate, displayNumber, displayPercent, calculateMemberBarWidth, isChallengeStarted, isSelectedUserId, formatNameAndEmail,
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

function setSelectedTabFromHashString(hashStr) {
    const hashParams = hashStr ? new URLSearchParams(hashStr.substring(1)) : new URLSearchParams()
    const hashSelectedTab = hashParams.get('tab')
    if (hashSelectedTab) {
        let tab = new bootstrap.Tab('#' + hashSelectedTab);
        tab.show()
    }
}

setSelectedTabFromHashString(window.location.hash)
window.addEventListener('hashchange', e => {
    setSelectedTabFromHashString(new URL(e.newURL).hash)
})


// Confetti on completion of a challenge
//
// const duration = 60 * 1000,
//   animationEnd = Date.now() + duration,
//   defaults = { startVelocity: 30, spread: 360, ticks: 60, zIndex: 0 };

// function randomInRange(min, max) {
//   return Math.random() * (max - min) + min;
// }

// const interval = setInterval(function() {
//   const timeLeft = animationEnd - Date.now();

//   if (timeLeft <= 0) {
//     return clearInterval(interval);
//   }

//   const particleCount = 15 * (timeLeft / duration);

//   // since particles fall down, start a bit higher than random
//   confetti(
//     Object.assign({}, defaults, {
//       particleCount,
//       scalar: 2,
//       origin: { x: randomInRange(0.1, 0.3), y: Math.random() - 0.2 },
//       shapes: ["emoji"],
//       shapeOptions: {
//         emoji: {
//             value: ["⭐️","👍","💪🏻","💪🏾","💪","❤️", "🌈","💚","💙"]
//         }
//       }
//     })
//   );
//   confetti(
//     Object.assign({}, defaults, {
//       particleCount,
//       scalar: 2,
//       origin: { x: randomInRange(0.7, 0.9), y: Math.random() - 0.2 },
//       shapes: ["emoji"],
//       shapeOptions: {
//         emoji: {
//             value: ["⭐️","👍","💪🏻","💪🏾","💪","❤️", "🌈","💚","💙"]
//         }
//       }
//     })
//   );
// }, 300);