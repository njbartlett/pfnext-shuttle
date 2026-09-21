import { createRequire } from 'node:module'
import { dirname } from 'node:path'
import { fileURLToPath, URL } from 'node:url'
import { defineConfig, normalizePath } from 'vite'
import vue from '@vitejs/plugin-vue'
import { viteStaticCopy } from 'vite-plugin-static-copy'

// Everything Rocket does not handle itself is proxied to it during
// development: the API and the images embedded in blog posts (/blog/blobs).
// `npm run dev` therefore needs `cargo run` on port 8000 alongside it.
const ROCKET_URL = 'http://localhost:8000'

// TinyMCE's skins (editor chrome and content styles) are plain CSS files
// the editor loads by URL, so they are copied rather than bundled; the
// editor is told where in src/views/EditPostView.vue
const require = createRequire(import.meta.url)
const TINYMCE_DIR = normalizePath(dirname(require.resolve('tinymce/package.json')))

export default defineConfig({
  plugins: [
    vue({
      template: {
        // Absolute /img/... URLs in templates refer to files in public/,
        // copied as-is rather than bundled
        transformAssetUrls: { includeAbsolute: false }
      }
    }),
    viteStaticCopy({
      // The plugin keeps each file's path relative to the project root, i.e.
      // node_modules/tinymce/skins/...; stripping the first two segments
      // gives dist/tinymce/skins/...
      targets: [{ src: `${TINYMCE_DIR}/skins`, dest: 'tinymce', rename: { stripBase: 2 } }]
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
      '/blog/blobs': ROCKET_URL
    }
  }
})
