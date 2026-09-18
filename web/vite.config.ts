import { readdirSync } from 'node:fs'
import { basename, resolve } from 'node:path'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// One bundle entry per file in src/pages/. Each entry corresponds to a page
// in templates/pages.toml marked `module = true`, and is loaded by
// templates/base_layout.html.tera as /js/pages/<template_name>.js.
const pagesDir = fileURLToPath(new URL('./src/pages', import.meta.url))
const entries = Object.fromEntries(
  readdirSync(pagesDir)
    .filter((file) => file.endsWith('.ts'))
    .map((file) => [basename(file, '.ts'), resolve(pagesDir, file)])
)

export default defineConfig({
  plugins: [
    vue({
      template: {
        // Absolute /img/... and similar URLs in templates are served by Rocket
        // from static/, not bundled assets
        transformAssetUrls: { includeAbsolute: false }
      }
    })
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    }
  },
  // Emitted URLs (chunk imports) are absolute from here, matching where
  // Rocket serves the static/ directory
  base: '/js/pages/',
  build: {
    outDir: '../static/js/pages',
    emptyOutDir: true,
    modulePreload: { polyfill: false },
    rollupOptions: {
      input: entries,
      output: {
        // Stable, un-hashed entry names so the Tera layout can address them;
        // shared code between pages lands in hashed chunks
        entryFileNames: '[name].js',
        chunkFileNames: 'chunks/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash][extname]'
      }
    }
  }
})
