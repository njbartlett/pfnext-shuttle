//const SERVER_URL = "http://localhost:8000"
const SERVER_URL = "https://pfnext.shuttleapp.rs"
const REDIRECT_TIMEOUT = 1000

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
        // dateStyle: "medium",
        weekday: "short",
        day: 'numeric',
        month: 'long'
    });
}

function displayTime(datetimestr) {
    const datetime = new Date(datetimestr);
    return datetime.toLocaleTimeString("en-GB", {
        timeStyle: "short",
        hour12: false
    });
}

function onLogout() {
    localStorage.removeItem("jwt")
    window.location.href = 'login.html'
}

function isAuthorizedSessionAdmin(auth) {
    return auth && (auth.roles.includes('admin') || auth.roles.includes('trainer'))
}

async function getValidatedAuthToken() {
    let jwtRaw = localStorage.getItem("jwt")
    if (!jwtRaw) {
        return null
    }
    let jwtJson = JSON.parse(jwtRaw);
    if (!jwtJson || !jwtJson.access_token) {
        return null
    }
    return fetch(SERVER_URL + "/validate_login", {
        method: "GET",
        headers: {
            'Authorization': 'Bearer ' + jwtJson.access_token
        },
        withCredentials: true,
        credentials: 'include'
    }).then(res => {
        if (!res.ok) throw res
        return jwtJson
    }).catch(err => {
        localStorage.removeItem("jwt")
        return null
    })
}
