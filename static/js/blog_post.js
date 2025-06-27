const postMeta = ref({
    id: null,
    title: null,
})

function afterLogout() {}

function isEditor() {
    if (loggedin.value && loggedin.value.roles) {
        return loggedin.value.roles.includes('admin') || loggedin.value.roles.includes('editor')
    }
    return false
}

function confirmedDeletePost() {
    fetch('/blog/posts/' + postMeta.value.id, {
        method: 'DELETE',
        credentials: 'include'
    }).then(res => {
        if (!res.ok) throw res
        window.location.href = '/blog/index.html?mode=allposts'
    }).catch(handleHttpError)
}

let app = createApp({
    setup() {
        return {
            loggedin, onLogout, isAdmin, isEditor, http_err, postMeta, confirmedDeletePost
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')
