// Entry point helper for module pages: every file in src/pages/ calls
// mountPage(SomeView) and nothing else. The shell supplies the navbar and
// the API error banner that templates/default_layout.html.tera used to.
import { createApp, type App, type Component } from 'vue'
import './api'
import PageShell from '@/components/PageShell.vue'
import { pageContextKey, readPageContext } from './pageContext'

export function mountPage(view: Component): App {
  const context = readPageContext()
  const app = createApp(PageShell, { view })
  app.provide(pageContextKey, context)
  app.mount('#app')
  return app
}
