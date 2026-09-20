// Environment for the end-to-end run: the repo's .env supplies the Postgres
// server; the tests use their own database on it so the development data is
// never touched.
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

export const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url))
export const E2E_PORT = 8010
export const BASE_URL = `http://localhost:${E2E_PORT}`
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
// E2E_DATABASE_URL to point somewhere else entirely
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
