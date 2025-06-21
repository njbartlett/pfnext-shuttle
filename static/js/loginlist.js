const logins = ref([])

async function loadLoginSessions() {
    httpGetJson("/loginsession", json => logins.value = json)
}

async function deleteLogin(login) {
    return httpCall("/loginsession/" + encodeURIComponent(login.sessionid), 'DELETE', null, loadLoginSessions)
}

let app = createApp({
    setup() {
        return {
            loggedin, http_err, logins, deleteLogin, TIMEZONE,
            isAdmin, encodeLoginReturnUrl, displayDateTime, onLogout
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

loadLoginSessions()