// End-to-end tests against the real stack: Rocket (built from source, on its
// own port) serving web/dist, with a dedicated Postgres database recreated
// from schema.sql and the Rust test fixtures on every run.
import { defineConfig, devices } from '@playwright/test'
import { BASE_URL, E2E_PORT, REPO_ROOT, testDatabaseUrl } from './e2e/env'

export default defineConfig({
  testDir: 'e2e',
  // Specs share one database and one member account, so run them in order
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list']],
  use: {
    baseURL: BASE_URL,
    trace: 'retain-on-failure'
  },
  projects: [
    { name: 'setup', testMatch: /.*\.setup\.ts/ },
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
      dependencies: ['setup']
    }
  ],
  webServer: {
    command: 'node web/e2e/reset-db.mjs && npm run build && cargo run',
    cwd: REPO_ROOT,
    url: `${BASE_URL}/index.html`,
    // A cold cargo build can take minutes
    timeout: 600_000,
    reuseExistingServer: false,
    stdout: 'ignore',
    stderr: 'pipe',
    env: {
      ...process.env,
      DATABASE_URL: testDatabaseUrl(),
      ROCKET_PORT: String(E2E_PORT),
      COOKIE_SECURE: 'false',
      // Keep the Rocket log readable when the server fails to start
      RUST_LOG: process.env.RUST_LOG ?? 'warn'
    }
  }
})
