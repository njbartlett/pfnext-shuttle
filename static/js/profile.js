// {% if jekyll.environment == "production" %}
// import { isEqual } from './js/underscore-esm-min.js'
// {% else %}
// import { isEqual } from './js/underscore-esm.js'
// {% endif %}

const user_data_orig = ref(null)
const user_data = reactive({
    id: 0,
    name: null,
    email: null,
    phone: null,
    emergency_name: null,
    emergency_phone: null,
    medical_info: null,
    roles: [],
    credits: 0
})
const user_data_changed = ref(false)
const reset_password_message = ref({
    message: null,
    is_error: false
})
const delete_profile = ref({
    password: "",
    unlock: false
})
const delete_profile_message = ref({
    message: null,
    is_error: false
})

function isEqual(user1, user2) {
    for (let key in user1) {
        if (user1.hasOwnProperty(key)) {
            let val1 = user1[key]
            let val2 = user2[key]
            if (val1 instanceof Array) {
                if (!(val2 instanceof Array)) {
                    return false
                }
                if (val1.length != val2.length) {
                    return false
                }
                for (let i = 0; i < val1.length; i++) {
                    if (val1[i] != val2[i]) {
                        return false
                    }
                }
            } else if (user1[key] != user2[key]) {
                return false
            }
        }
    }
    return true
}

function copyUserFields(from, to) {
    for (let key in from) {
        let val = from[key]
        if (val instanceof Array) {
            to[key] = from[key].slice() // clone array
        } else {
            to[key] = from[key]
        }
    }
}

async function loadUserDetails() {
    httpGetJson("/users/" + loggedin.value.id, user => {
        user_data_orig.value = user
        copyUserFields(user, user_data)
    })
}

async function onResetPassword() {
    reset_password_message.value = {
        message: "Processing...",
        is_error: false
    }
    httpCall("/request_pwd_reset", "POST", {
        email: loggedin.value.email,
        website_url: document.location.host,
        reset_url: new URL("/passwordreset.html", document.location).href
    }, res => res.text().then(text => {
        reset_password_message.value = {
            message: text,
            is_error: false
        }
        window.location.href = "/passwordreset.html?email=" + encodeURIComponent(loggedin.value.email)
    }))
}

// async function onDeleteProfile() {
//     delete_profile_message.value = {
//         message: "Processing...",
//         is_error: false
//     }
//     httpCall
//     fetch(SERVER_URL + "/users/" + loggedin.value.id, {
//         method: "DELETE",
//         headers: {
//             "Content-Type": "application/json"
//         },
//         body: JSON.stringify({
//             password: delete_profile.value.password,
//             website_url: document.location.host
//         })
//     }).then(res => {
//         if (!res.ok) throw res
//     }).then(_ => {
//         delete_profile_message.value = {
//             message: "Profile deleted. Redirecting to home page.",
//             is_error: false
//         }
//         localStorage.removeItem("jwt")
//         setTimeout(_ => window.location.href = "/", 1000)
//     }).catch(err => err.text().then(err_text => {
//         delete_profile_message.value = {
//             message: err_text,
//             is_error: true
//         }
//     }))
// }

async function doSave() {
    httpCall("/users/" + loggedin.value.id, "PATCH", {
            name: user_data.name,
        phone: user_data.phone,
        emergency_name: user_data.emergency_name,
        emergency_phone: user_data.emergency_phone,
        medical_info: user_data.medical_info
    }, loadUserDetails)
}

let app = createApp({
    setup() {
        return {
            loggedin, user_data, user_data_changed, http_err, reset_password_message, delete_profile, delete_profile_message, doSave,
            onResetPassword, onLogout, isAdmin, encodeLoginReturnUrl
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

loadUserDetails().then(_ => 
    watch(user_data, new_user_data => {
        user_data_changed.value = !isEqual(new_user_data, user_data_orig.value)
    })
)

