import { defineConfig } from 'vitest/config'

export default defineConfig({
  test: {
    // The API client reads window.location; the format helpers use Intl
    environment: 'jsdom',
    // Browser-zone helpers must give the same answers on every machine
    env: { TZ: 'UTC' },
    include: ['src/**/*.test.ts']
  }
})
