// End-to-end tests against the real stack: Rocket (built from source, on its
// own port) serving web/dist, with a dedicated Postgres database recreated
// from schema.sql and the Rust test fixtures on every run.
import { defineConfig, devices } from '@playwright/test'
import { BASE_URL, REPO_ROOT, rocketEnv } from './e2e/env'

// The web app is built as part of starting the server unless it has been
// built already (CI builds it once, for the unit tests and for this)
const buildStep = process.env.E2E_SKIP_WEB_BUILD ? '' : 'npm run build && '

export default defineConfig({
  testDir: 'e2e',
  // Specs share one database and one member account, so run them in order
  fullyParallel: false,
  workers: 1,
  retries: 0,
  forbidOnly: !!process.env.CI,
  reporter: process.env.CI ? [['list'], ['github']] : [['list']],
  use: {
    baseURL: BASE_URL,
    trace: 'retain-on-failure',
    // The club's time zone and locale, whatever machine the tests run on
    timezoneId: 'Europe/London',
    locale: 'en-GB'
  },
  projects: [
    { name: 'setup', testMatch: /.*\.setup\.ts/ },
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
      dependencies: ['setup']
    }
  ],
  // With E2E_BASE_URL the stack under test is already running
  webServer: process.env.E2E_BASE_URL
    ? undefined
    : {
        command: `node web/e2e/reset-db.mjs && ${buildStep}cargo run`,
        cwd: REPO_ROOT,
        url: `${BASE_URL}/index.html`,
        // A cold cargo build can take minutes
        timeout: 600_000,
        reuseExistingServer: false,
        stdout: 'ignore',
        stderr: 'pipe',
        env: rocketEnv()
      }
})
