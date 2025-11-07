const polls_data = ref([])
const admin_controls = reactive({
    all_members: false
})
const admin_all_members = ref([])

function loadPollsWithVotes() {
    let params = new URLSearchParams()
    if (!admin_controls.all_members) {
        params.set("person_id", loggedin.value.id)
    }
    httpGetJson("/polls?" + params.toString(), json => {
        polls_data.value = json
    })
}

function adminSubmitVote(poll) {
    poll.new_vote = poll.admin_new_vote
    poll.admin_new_vote = null

    let person = null
    for (user of admin_all_members.value) {
        if (formatNameAndEmail(user.name, user.email) === poll.admin_new_vote_person) {
            person = user
            break
        }
    }
    if (person) {
        poll.admin_new_vote_person = null
        submitVote(person.id, poll)
    } else {
        alert(`Could not find user matching "${person_name_email}". Please select a valid user from the list.`)
    }
}

function submitVote(person_id, poll) {
    const vote_body = {
        person_id: person_id,
        poll_id: poll.poll.id,
        value: poll.new_vote
    }
    poll.new_vote = null
    httpCall("/votes", 'POST', vote_body, resp => {
        resp.json().then(vote => {            
            console.log("Submitted vote, got response: ", vote)
            poll.votes.push(vote)
        })
    })
}

function removeVote(poll, vote) {
    httpCall(`/votes/${vote.id}`, 'DELETE', null, resp => {
        console.log(`Deleted vote ${vote.id}`)
        // Remove vote from poll's votes
        let vote_index = poll.votes.findIndex(v => v.id === vote.id)
        if (vote_index !== -1) {
            poll.votes.splice(vote_index, 1)
        }
    })
}

function exportPollsCSV(poll) {
    let csv_content = "Value,Voter Name,Voter Email"
    for (vote of poll.votes) {
        let quoted_value = `"${vote.value.replace(/"/g, '""')}"`
        let row = `${quoted_value},${vote.person_name},${vote.person_email}`
        csv_content += `\n${row}`
    }
    const blob = new Blob([csv_content], {type: 'text/csv'})
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url;
    title = poll.poll.question.replace(/\s+/g, '_')
    a.download = `${title}_votes.csv`
    a.click()
    URL.revokeObjectURL(url)
}

let app = createApp({
    setup() {
        return {
            // Data
            loggedin, http_err,
            polls_data, admin_controls, admin_all_members,

            // Functions
            isAdmin, onLogout, encodeLoginReturnUrl, removeVote, adminSubmitVote, submitVote, formatNameAndEmail, exportPollsCSV
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

loadPollsWithVotes()
watch(admin_controls, loadPollsWithVotes)

loadAllUsers(users => {
    admin_all_members.value = users
})