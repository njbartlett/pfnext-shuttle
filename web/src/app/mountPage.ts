// Entry point helper for module pages: every file in src/pages/ calls
// mountPage(SomeView) and nothing else. The shell supplies the navbar and
// the API error banner that templates/default_layout.html.tera used to.
import { createApp, type App, type Component } from 'vue'
import './api'
import PageShell from '@/components/PageShell.vue'
import { configureLogout } from '@/stores/auth'
import { pageContextKey, readPageContext } from './pageContext'

export interface MountOptions {
  // false for the focused pages (login, registration, editors) that the
  // legacy nonav_layout rendered without the navbar
  navbar?: boolean
  // true for pages that work logged out, so Logout keeps the user on them
  stayOnLogout?: boolean
}

export function mountPage(view: Component, options: MountOptions = {}): App {
  const context = readPageContext()
  configureLogout({ stay: options.stayOnLogout ?? false })
  const app = createApp(PageShell, { view, navbar: options.navbar ?? true })
  app.provide(pageContextKey, context)
  app.mount('#app')
  return app
}
