function afterLogout() {}

function isEditor() {
    if (loggedin.value && loggedin.value.roles) {
        return loggedin.value.roles.includes('admin') || loggedin.value.roles.includes('editor')
    }
    return false
}

let app = createApp({
    setup() {
        return {
            loggedin, onLogout, isAdmin, isEditor, http_err
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')
