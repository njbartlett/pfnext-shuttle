import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Everything Rocket does not handle itself is proxied to it during
// development: the API, the Tera-rendered blog and the legacy assets the blog
// pages load (static/js, static/styles). `npm run dev` therefore needs
// `cargo run` on port 8000 alongside it.
const ROCKET_URL = 'http://localhost:8000'

export default defineConfig({
  plugins: [
    vue({
      template: {
        // Absolute /img/... URLs in templates refer to files in public/,
        // copied as-is rather than bundled
        transformAssetUrls: { includeAbsolute: false }
      }
    })
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    }
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true
  },
  server: {
    proxy: {
      '/api': ROCKET_URL,
      '/blog': ROCKET_URL,
      '/js': ROCKET_URL,
      '/styles': ROCKET_URL
    }
  }
})
