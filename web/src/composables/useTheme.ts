// Bootstrap colour mode. The choice is stored under the same key that the
// inline script in index.html reads before first paint, and that
// static/js/theme.js uses for the Tera blog pages.
import { readonly, ref } from 'vue'

export type Theme = 'dark' | 'light'

const THEME_STORAGE_KEY = 'theme'
const THEME_ATTRIBUTE = 'data-bs-theme'

function isTheme(value: unknown): value is Theme {
  return value === 'dark' || value === 'light'
}

function storedTheme(): Theme | null {
  try {
    const value = localStorage.getItem(THEME_STORAGE_KEY)
    return isTheme(value) ? value : null
  } catch {
    return null
  }
}

function systemTheme(): Theme {
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

function applyTheme(theme: Theme) {
  document.documentElement.setAttribute(THEME_ATTRIBUTE, theme)
}

const theme = ref<Theme>(storedTheme() ?? systemTheme())
applyTheme(theme.value)

// Follow the system preference while the user has not chosen explicitly
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  if (storedTheme() === null) {
    theme.value = systemTheme()
    applyTheme(theme.value)
  }
})

export function setTheme(newTheme: Theme) {
  theme.value = newTheme
  applyTheme(newTheme)
  try {
    localStorage.setItem(THEME_STORAGE_KEY, newTheme)
  } catch {
    // Preference simply not remembered
  }
}

export function useTheme() {
  return { theme: readonly(theme), setTheme }
}
