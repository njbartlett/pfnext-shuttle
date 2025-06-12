const user_data = reactive({
    name: "",
    email: "",
    phone: "",
    emergency_name: "",
    emergency_phone: "",
    medical_info: "",
    terms_agreed: false,
})
const user_data_val = ref({
    name: null,
    email: null,
    phone: null,
    emergency_name: "",
    emergency_phone: "",
    terms_agreed: null,
    ready: false
})
const register_result = ref(null)

async function onSubmit() {
    register_result.value = {
        message: "Processing...",
        is_error: false,
        is_existing_user: false
    }
    fetch(SERVER_URL + "/register_user", {
        method: "POST",
        headers: {"Content-Type": "application/json"},
        body: JSON.stringify({
            name: user_data.name,
            email: user_data.email,
            phone: user_data.phone,
            emergency_name: user_data.emergency_name,
            emergency_phone: user_data.emergency_phone,
            medical_info: user_data.medical_info,
            website_url: document.location.host,
            reset_url: new URL("/passwordreset.html", document.location).href
        })
    }).then(res => {
        if (!res.ok) throw res
        register_result.value = {
            message: "Registration email sent!",
            is_error: false,
            is_existing_user: false
        }
        window.location.href = "/passwordreset.html?email=" + encodeURIComponent(user_data.email)
    }).catch(error => {
        error.text().then(error_text => {
            register_result.value = {
                message:  error_text,
                is_error: true,
                is_existing_user: error.status === 409 // Conflict
            }
        })
    })
}

watch(user_data, async (new_data, old_data) => {
    let val = user_data_val.value
    val.name = new_data.name ? null : "Name must not be empty"
    val.email = validateEmail(new_data.email) ? null : "Enter a valid email address"
    val.phone = validatePhone(new_data.phone) ? null : "Enter a valid phone number without space or parentheses (country code optional)"
    val.emergency_name = new_data.emergency_name ? null : "Emergency contact name must not be empty"
    val.emergency_phone = validatePhone(new_data.emergency_phone) ? null : "Enter a valid phone number without space or parentheses (country code optional)"
    val.terms_agreed = new_data.terms_agreed ? null : "You must agree to the terms and conditions before registering"

    val.ready = val.name === null &&
        val.email === null &&
        val.phone === null &&
        val.emergency_name === null &&
        val.emergency_phone === null &&
        val.terms_agreed === null
})

let app = createApp({
    setup() {
        return {
            // Data
            user_data, user_data_val, register_result, http_err,
            // Callback functions
            onSubmit, isAdmin, goBack
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount("#app")
