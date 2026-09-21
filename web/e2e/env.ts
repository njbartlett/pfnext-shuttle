// Environment for the end-to-end run: the repo's .env supplies the Postgres
// server; the tests use their own database on it so the development data is
// never touched.
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

export const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url))
export const E2E_PORT = 8010
// Set E2E_BASE_URL to test a stack you are running yourself (for example the
// Vite dev server proxying to Rocket); Playwright then starts nothing
export const BASE_URL = process.env.E2E_BASE_URL ?? `http://localhost:${E2E_PORT}`
export const TEST_DATABASE_NAME = 'pfnext_test'

export function loadRepoEnv(): Record<string, string> {
  const values: Record<string, string> = {}
  let text = ''
  try {
    text = readFileSync(new URL('../../.env', import.meta.url), 'utf8')
  } catch {
    return values
  }
  for (const line of text.split('\n')) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) {
      continue
    }
    const separator = trimmed.indexOf('=')
    if (separator > 0) {
      values[trimmed.slice(0, separator).trim()] = trimmed.slice(separator + 1).trim().replace(/^["']|["']$/g, '')
    }
  }
  return values
}

// The test database lives on the same server as the development one; set
// E2E_DATABASE_URL to point somewhere else entirely (CI does)
export function testDatabaseUrl(): string {
  if (process.env.E2E_DATABASE_URL) {
    return process.env.E2E_DATABASE_URL
  }
  const devUrl = process.env.DATABASE_URL ?? loadRepoEnv().DATABASE_URL
  if (!devUrl) {
    throw new Error('DATABASE_URL not found in the environment or .env')
  }
  const url = new URL(devUrl)
  url.pathname = `/${TEST_DATABASE_NAME}`
  return url.toString()
}

// Rocket refuses to start without these (src/config.rs). Locally .env has
// them; where it does not (CI) test-only values do, since no test sends
// email or issues mobile tokens. The secret key is a random 256-bit value
// used for nothing but this test server.
const ROCKET_DEFAULTS: Record<string, string> = {
  ACCESS_TOKEN_KEY: 'e2e-access-token-key',
  REFRESH_TOKEN_KEY: 'e2e-refresh-token-key',
  SMTP_USERNAME: 'e2e',
  SMTP_PASSWORD: 'e2e',
  CORS_ALLOWED: 'http://localhost',
  ROCKET_SECRET_KEY: 'epRqI/D+MDJlaxu/tdN/p5pJPo9s9F5LAHljKkDsgxc='
}

// The environment Rocket is started with for the tests
export function rocketEnv(): Record<string, string> {
  const merged: Record<string, string | undefined> = {
    ...ROCKET_DEFAULTS,
    ...loadRepoEnv(),
    ...process.env,
    DATABASE_URL: testDatabaseUrl(),
    ROCKET_ADDRESS: '127.0.0.1',
    ROCKET_PORT: String(E2E_PORT),
    COOKIE_SECURE: 'false',
    // Keep the Rocket log readable when the server fails to start
    RUST_LOG: process.env.RUST_LOG ?? 'warn'
  }
  return Object.fromEntries(Object.entries(merged).filter((entry): entry is [string, string] => entry[1] !== undefined))
}
