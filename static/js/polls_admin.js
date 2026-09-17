// All polls with their votes (admin view, all members)
const polls_data = ref([])

// Poll currently being created (id === null) or edited
const edit_poll = reactive({
    id: null,
    question: '',
    description: '',
    limit_per_person: 1,
    open: true
})
const edit_poll_validation = ref({
    question: null,
    limit_per_person: null,
    ready: false
})
const outcome = ref(null)
const deleting_poll = ref(null)

function loadPolls() {
    if (!isAdmin()) return
    // The admin list must always include closed polls, which /polls omits by default
    httpGetJson("/polls?closed=true", json => {
        polls_data.value = json
    })
}

function setEditPoll(poll) {
    edit_poll.id = poll ? poll.id : null
    edit_poll.question = poll ? poll.question : ''
    edit_poll.description = poll && poll.description ? poll.description : ''
    edit_poll.limit_per_person = poll ? poll.limit_per_person : 1
    edit_poll.open = poll ? poll.open : true
}

// Reset the form to create a new poll
function newPoll() {
    setEditPoll(null)
    outcome.value = null
}

// Load an existing poll into the form for editing
function onEditPoll(poll) {
    setEditPoll(poll)
    outcome.value = null
}

// Build the request body expected by POST/PUT /polls
function pollRequestBody(poll) {
    let description = poll.description ? poll.description.trim() : ''
    return {
        question: poll.question ? poll.question.trim() : '',
        description: description.length > 0 ? description : null,
        limit_per_person: Number(poll.limit_per_person),
        open: !!poll.open
    }
}

function savePoll() {
    if (!edit_poll_validation.value.ready) return

    const is_new = edit_poll.id === null
    const url = is_new ? "/polls" : `/polls/${edit_poll.id}`
    httpCall(url, is_new ? 'POST' : 'PUT', pollRequestBody(edit_poll), res => {
        return res.json().then(saved => {
            console.log(is_new ? "Created poll: " : "Updated poll: ", saved)
            if (is_new) {
                newPoll()
                outcome.value = { is_error: false, message: `Created poll "${saved.question}"` }
            } else {
                setEditPoll(saved)
                outcome.value = { is_error: false, message: "Saved!" }
            }
            loadPolls()
        })
    })
}

// Open or close a poll directly from the list, keeping its other fields unchanged
function setPollOpen(poll, open) {
    let body = pollRequestBody(poll)
    body.open = open
    httpCall(`/polls/${poll.id}`, 'PUT', body, res => {
        return res.json().then(saved => {
            console.log(`Set poll ${saved.id} open=${saved.open}`)
            if (edit_poll.id === saved.id) {
                edit_poll.open = saved.open
            }
            loadPolls()
        })
    })
}

function onClickDeletePoll(poll_with_votes) {
    deleting_poll.value = poll_with_votes
}

function deletePoll(poll_with_votes) {
    const poll_id = poll_with_votes.poll.id
    httpCall(`/polls/${poll_id}`, 'DELETE', null, _ => {
        console.log(`Deleted poll ${poll_id}`)
        if (edit_poll.id === poll_id) {
            newPoll()
        }
        deleting_poll.value = null
        loadPolls()
    })
}

function validateEditPoll(poll, val) {
    // A completely blank question shows no error (the form is just empty), but
    // whitespace-only is called out. Either way the form is not ready.
    const question_blank = !poll.question || poll.question.length === 0
    const question_ok = !question_blank && poll.question.trim().length > 0
    val.question = question_ok || question_blank ? null : "Question is required"

    const limit = Number(poll.limit_per_person)
    const limit_ok = poll.limit_per_person !== null && poll.limit_per_person !== '' && Number.isInteger(limit) && limit >= 1
    val.limit_per_person = limit_ok ? null : "Must be a whole number, at least 1"

    val.ready = question_ok && limit_ok
}

let app = createApp({
    setup() {
        return {
            // Data
            loggedin, http_err,
            polls_data, edit_poll, edit_poll_validation, outcome, deleting_poll,

            // Functions
            isAdmin, onLogout, encodeLoginReturnUrl,
            newPoll, onEditPoll, savePoll, setPollOpen, onClickDeletePoll, deletePoll
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

watch(edit_poll, poll => validateEditPoll(poll, edit_poll_validation.value))

loadPolls()
