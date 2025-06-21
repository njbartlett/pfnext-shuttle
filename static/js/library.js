// BEGIN library.js
// {% if jekyll.environment == "production" %}
// import { createApp, onMounted, reactive, ref, watch } from '/static/js/vue.esm-browser.prod.js'
// {% else %}
// import { createApp, onMounted, reactive, ref, watch } from '/static/js/vue.esm-browser.js'
// {% endif %}

const { createApp, onMounted, reactive, ref, watch } = Vue

const SERVER_URL = '/api'
const LOGIN_STORAGE_KEY = "anotherlevellogin"
const TIMEZONE = "Europe/London"

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

async function httpCall(url, method, body, onres) {
    let request = {
        method: method,
        credentials: 'include'
    }
    if (body) {
        request.headers = {
            'Content-Type': 'application/json'
        }
        request.body = JSON.stringify(body)
    }
    return fetch(SERVER_URL + url, request).then(res => {
        if (!res.ok) throw res
        if (onres != null) {
            return onres(res)
        } else {
            return res
        }
    }).catch(handleHttpError)
}

async function httpGetJson(url, onjson) {
    return httpCall(url, 'GET', null, res => {
        return res.json().then(onjson)
    })
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
            http_err.value.text = error_text
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
            window.location.href = 'index.html'
        }
    })
}

function isAdmin() {
    return loggedin.value && loggedin.value.roles && loggedin.value.roles.includes('admin')
}

function isTrainer() {
    return loggedin.value && loggedin.value.roles && loggedin.value.roles.includes('trainer')
}

function validateEmail(email) {
    return String(email)
        .toLowerCase()
        .match(
            /^(([^<>()[\]\\.,;:\s@"]+(\.[^<>()[\]\\.,;:\s@"]+)*)|.(".+"))@((\[[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\])|(([a-zA-Z\-0-9]+\.)+[a-zA-Z]{2,}))$/
        );
}

function validatePhone(phone) {
    return String(phone).match(/^\+?\d+$/)
}

function displayDate(datestr) {
    return new Date(datestr).toLocaleDateString("en-GB", {
        timeZone: TIMEZONE,
        weekday: "short",
        day: 'numeric',
        month: 'long',
        year: 'numeric'
    })
}

function displayTime(datetimestr) {
    const datetime = new Date(datetimestr)
    return datetime.toLocaleTimeString("en-GB", {
        timeZone: TIMEZONE,
        timeStyle: "short",
        hour12: false
    })
}

function displayDateTime(datetimestr) {
    const datetime = new Date(datetimestr)
    return datetime.toLocaleDateString("en-GB", {
        timeZone: TIMEZONE,
        day: 'numeric',
        month: 'short',
        year: 'numeric'
    }) + ' ' + datetime.toLocaleTimeString("en-GB", {
        timeZone: TIMEZONE
    })
}

function displayNumber(num) {
    return num.toLocaleString("en-GB")
}

function displayPercent(num, places) {
    return (num).toLocaleString(undefined, {
        style: 'percent',
        minimumFractionDigits: places,
        maximumFractionDigits: places
    })
}

function formatNameAndEmail(name, email) {
    return name + ' <' + email + '>'
}

function findUserByEmail(email, all_users) {
    for (var user of all_users) {
        if (user.email === email) {
            return user
        }
    }
    return null
}

function encodeLoginReturnUrl() {
    return encodeURIComponent(window.location.pathname + window.location.hash)
}

function startOfWeek(date) {
    let start = new Date(date)
    let currentDay = start.getDay()

    let offsetDays = (currentDay + 7 - START_OF_WEEK) % 7
    start.setDate(start.getDate() - offsetDays)
    start.setHours(0, 0, 0, 0) // Set to midnight of that day
    return start
}

function startOfMonth(date) {
    let d = new Date()
    d.setYear(date.getFullYear())
    d.setMonth(date.getMonth())
    d.setDate(1)
    d.setHours(0, 0, 0, 0) // Set to midnight of that day
    return d
}

function endOfMonth(date) {
    let d = new Date()
    d.setYear(date.getFullYear())
    d.setMonth(date.getMonth() + 1)
    d.setDate(0)
    d.setHours(23, 59, 59, 999) // Set to 1ms before midnight of that day
    return d
}

function addDays(date, days) {
    var result = new Date(date)
    result.setDate(result.getDate() + days)
    return result
}

function isPast(datetime) {
    return new Date() > new Date(datetime)
}

// Get a list of all users and pass this to the callback.
// When the user is not an admin, the list passed is only the current user.
async function loadAllUsers(callback) {
    if (isAdmin()) {
        return httpGetJson("/users/list", callback)
    } else if (loggedin.value) {
        return callback([loggedin.value])
    } else {
        return callback(null)
    }
}

function goBack() {
    if (typeof page_return_path !== 'undefined' && page_return_path !== null && page_return_path.value != null) {
        window.location.href = page_return_path.value
    } else {
        window.history.back()
    }

}

// END library.js