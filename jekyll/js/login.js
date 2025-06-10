const url_params = new URLSearchParams(window.location.search)
const user_data = reactive({
    email: url_params.get("email"),
    password: "",
    show_forgotten: false
})
const user_data_val = ref({
    email: null,
    ready: false
})

const reset_password_result = ref(null)
const login_return_path = ref("index.html")

async function onLogin() {
    http_err.value = null
    setAuthenticatedUser(null)
    httpCall("/login", "POST", {
        email: user_data.email,
        password: user_data.password
    }, res => res.json().then(json => {
        setAuthenticatedUser(json)
        window.location.href = login_return_path.value
    }))
}

async function onResetPassword() {
    reset_password_result.value = {
        message:  "Processing...",
        is_error: false
    }
    httpCall("/request_pwd_reset", "POST", {
        email: user_data.email,
        website_url: document.location.host,
        reset_url: new URL("/passwordreset.html", document.location).href
    }, res => res.text().then(text => {
        reset_password_result.value = {
            message: text,
            is_error: false
        }
        window.location.href = "/passwordreset.html?email=" + encodeURIComponent(user_data.email)
    }))
}

async function onForgottenPassword() {
    user_data.show_forgotten = true
}

async function onUnforgottenPassword() {
    user_data.show_forgotten = false
}

watch(user_data, async (new_data, old_data) => {
    let val = user_data_val.value
    val.email = validateEmail(new_data.email) ? null : "Enter a valid email address"

    val.ready = val.email === null
})

let app = createApp({
    setup() {
        return {
            // Data
            loggedin, user_data, user_data_val, http_err, reset_password_result,
            // Callbacks
            onLogin, onForgottenPassword, onUnforgottenPassword, onResetPassword, onLogout, isAdmin
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

let return_path = url_params.get("return")
if (return_path) {
    login_return_path.value = decodeURIComponent(return_path)
}

