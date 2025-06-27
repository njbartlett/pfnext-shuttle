const postMeta = reactive({
    postId: null,
    title: '',
    pubDate: null,
    pubTime: '00:00'
})
const saveStatus = ref({
    dirty: false,
    message: null
})
const postMetaValidation = ref({
    title: null,
    ready: false
})

function isEditor() {
    if (loggedin.value && loggedin.value.roles) {
        return loggedin.value.roles.includes('admin') || loggedin.value.roles.includes('editor')
    }
    return false
}

function cancelEditing() {
    window.history.back()
}

async function savePost() {
    const html = tinymce.activeEditor.getContent()
    const body = {
        title: postMeta.title,
        content: html,
    }
    if (postMeta.pubDate != null && postMeta.pubDate !== '' && postMeta.pubTime != null && postMeta.pubTime !== '') {
        const datetime = new Date(postMeta.pubDate + " " + postMeta.pubTime)
        body.published = datetime.toISOString()
    }

    const baseUrl = '/blog/posts'
    if (postMeta.postId != null) {
        return fetch(baseUrl + '/' + postMeta.postId, {
            method: 'PUT',
            credentials: 'include',
            body: JSON.stringify(body)
        }).then(res => {
            if (!res.ok) throw res
            saveStatus.value = {
                dirty: false,
                message: 'Saved at ' + new Date().toLocaleString('en-GB', {})
            }
            let location = res.headers.get('Location')
            if (location != null) {
                window.location.href = location
            }
        }).catch(handleHttpError)
    } else {
        return fetch(baseUrl, {
            method: 'POST',
            credentials: 'include',
            body: JSON.stringify(body)
        }).then(res => {
            if (!res.ok) throw res
            window.location.href = res.headers.get('Location')
        }).catch(handleHttpError)
    }
}

function updatedPostMeta(newData, validation) {
    console.log("updatedPostMeta")
    saveStatus.value = {
        dirty: true,
        message: null
    }

    validation.title = (newData.title == null || newData.title === '') ? 'Title must not be empty.' : null
    validation.pubTime = (newData.pubDate != null && newData.pubTime == null) ? 'Publication time must be specified.' : null
    
    validation.ready =
        validation.title == null &&
        validation.pubTime == null
}

function editorContentChanged() {
    console.log("Editor content changed")
}

let app = createApp({
    setup() {
        return {
            // Data
            loggedin, onLogout, isAdmin, isEditor, http_err, postMeta, postMetaValidation, saveStatus,
            // Functions
            cancelEditing, savePost, editorContentChanged
        }
    }
})

app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')
