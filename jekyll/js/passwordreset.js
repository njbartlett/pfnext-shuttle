const REDIRECT_TIMEOUT = 1000
const url_params = new URLSearchParams(window.location.search)
const user_data = ref({
    email: url_params.get('email'),
    temp_password: url_params.get('temp_pwd'),
    new_password: "",
    new_password_repeat: "",
    validation_ok: false,
    validation_errs: []
})
const reset_password_result = ref(null)

function onChangePasswordInput() {
    var errs = []
    if (user_data.value.new_password === user_data.value.temp_password) {
        errs.push("New password cannot be the same as the temporary password")
    }
    if (user_data.value.new_password.length < 8) {
        errs.push("Password must be at least 8 characters long")
    }
    if (user_data.value.new_password !== user_data.value.new_password_repeat) {
        errs.push("Passwords do not match")
    }
    user_data.value.validation_errs = errs
    user_data.value.validation_ok = errs.length === 0
}

async function onSubmit() {
    reset_password_result.value = {
        message: "Processing...",
        is_error: false
    }
    httpCall("/reset_pwd", "POST", {
        email: user_data.value.email,
        temp_password: user_data.value.temp_password,
        new_password: user_data.value.new_password,
        website_url: document.location.host
    }, res => {
        setAuthenticatedUser(null) // Clear the current user, forcing a re-login
        reset_password_result.value = {
            message: "Password successfully updated! Redirecting back to login...",
            is_error: false
        }
        setTimeout((data => window.location.href = "/login.html?email=" + encodeURIComponent(user_data.value.email)), REDIRECT_TIMEOUT)
    })
}

let app = createApp({
    setup() {
        return {
            // Data
            loggedin, user_data, reset_password_result, http_err,

            // Callback functions
            onChangePasswordInput, onSubmit, onLogout, isAdmin
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount("#app")
