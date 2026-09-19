// Single entry point for the website. Bootstrap's stylesheet, icon font and
// JavaScript (the data-api behind dropdowns and collapses) come from npm;
// components that need a Bootstrap class import it from 'bootstrap' directly.
import 'bootstrap/dist/css/bootstrap.min.css'
import 'bootstrap-icons/font/bootstrap-icons.css'
import 'bootstrap'
import { createApp } from 'vue'
import { configureApi } from '@pfnext/shared'
import App from './App.vue'
import { router } from './router'

// Same-origin, unversioned API mount, authenticated by the session cookie
configureApi({ baseUrl: '/api', credentials: 'include' })

createApp(App).use(router).mount('#app')
