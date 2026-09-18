// BEGIN library.js
//
// Shared globals for the remaining legacy (Tera + global script) pages: the
// blog index, blog post and blog editor. Every other page has moved to the
// Vite/Vue build in web/, which has its own equivalents (web/src/stores).
// Keep LOGIN_STORAGE_KEY in step with web/src/stores/auth.ts.

const { createApp, onMounted, reactive, ref, watch, computed } = Vue

const SERVER_URL = '/api'
const LOGIN_STORAGE_KEY = "anotherlevellogin"

const http_err = ref(null)
const loggedin = ref((_ => {
    let loggedin_raw = localStorage.getItem(LOGIN_STORAGE_KEY)
    if (!loggedin_raw) {
        return null
    }
    return JSON.parse(loggedin_raw)
})())

function setAuthenticatedUser(user) {
    loggedin.value = user
    if (!user) {
        localStorage.removeItem(LOGIN_STORAGE_KEY)
    } else {
        localStorage.setItem(LOGIN_STORAGE_KEY, JSON.stringify(user))
    }
}

// API errors are JSON of the form {"code": "...", "message": "..."}; extract the
// human-readable message, falling back to the raw text for non-JSON bodies
function parseApiError(error_text) {
    try {
        const parsed = JSON.parse(error_text)
        if (parsed && parsed.message) {
            return parsed.message
        }
    } catch (e) {
        // Not JSON, fall through to the raw text
    }
    return error_text
}

async function handleHttpError(err) {
    if (err.status == 401) {
        setAuthenticatedUser(null)
    }
    http_err.value = {
        status: err.status
    }
    if (typeof err.text === 'function') {
        return err.text().then(error_text => {
            http_err.value.text = parseApiError(error_text)
        })
    } else {
        http_err.value.text = err.message
    }
}

function onLogout() {
    setAuthenticatedUser(null)
    fetch(SERVER_URL + "/logout", {
        method: "POST",
        credentials: 'include'
    }).then(_ => {
        if (typeof afterLogout === "function") {
            afterLogout()
        } else {
            window.location.href = '/'
        }
    })
}

function isAdmin() {
    return loggedin.value && loggedin.value.roles && loggedin.value.roles.includes('admin')
}

function encodeLoginReturnUrl() {
    return encodeURIComponent(window.location.pathname + window.location.hash)
}

// END library.js
