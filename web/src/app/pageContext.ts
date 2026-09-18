// The server-rendered page context. templates/base_layout.html.tera embeds
// the Tera `common` and `page` objects (src/templates.rs) as JSON in a
// <script type="application/json"> block for module pages.
import { inject, type InjectionKey } from 'vue'

export interface NavBarPage {
  title: string
  url: string
}

export interface CommonPageContext {
  branding: string
  navigation: NavBarPage[]
  prod: boolean
  // Path of the current page, e.g. "/index.html"
  url: string
}

export interface PageInfo {
  title: string
  template_name: string
}

export interface PageContext {
  common: CommonPageContext
  page: PageInfo
}

export const pageContextKey: InjectionKey<PageContext> = Symbol('pageContext')

export function readPageContext(): PageContext {
  const element = document.getElementById('page-context')
  if (!element?.textContent) {
    throw new Error('No #page-context block found; module pages must be rendered by templates/module_page.html.tera')
  }
  return JSON.parse(element.textContent) as PageContext
}

export function usePageContext(): PageContext {
  const context = inject(pageContextKey)
  if (!context) {
    throw new Error('Page context not provided; mount the page with mountPage()')
  }
  return context
}
