// Recreates the end-to-end test database from schema.sql and the SQL
// fixtures the Rust tests use, so every run starts from the same state.
// Run by the Playwright webServer command before Rocket starts.
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import pg from 'pg'

const REPO_ROOT = fileURLToPath(new URL('../..', import.meta.url))
const FIXTURES = ['src/fixtures/users.sql']

const databaseUrl = process.env.DATABASE_URL
if (!databaseUrl) {
  throw new Error('DATABASE_URL must be set to the test database')
}
const url = new URL(databaseUrl)
const databaseName = url.pathname.slice(1)
// Guard against ever resetting a development or production database
if (!databaseName.endsWith('_test')) {
  throw new Error(`Refusing to reset database "${databaseName}": the name must end in _test`)
}

async function ensureDatabase() {
  const adminUrl = new URL(url)
  adminUrl.pathname = '/postgres'
  const admin = new pg.Client({ connectionString: adminUrl.toString() })
  await admin.connect()
  try {
    const existing = await admin.query('SELECT 1 FROM pg_database WHERE datname = $1', [databaseName])
    if (existing.rowCount === 0) {
      await admin.query(`CREATE DATABASE "${databaseName}"`)
      console.log(`Created database ${databaseName}`)
    }
  } finally {
    await admin.end()
  }
}

async function resetSchema() {
  const client = new pg.Client({ connectionString: databaseUrl })
  await client.connect()
  try {
    await client.query('DROP SCHEMA public CASCADE; CREATE SCHEMA public;')
    await client.query(readFileSync(new URL('schema.sql', `file://${REPO_ROOT}`), 'utf8'))
    for (const fixture of FIXTURES) {
      await client.query(readFileSync(new URL(fixture, `file://${REPO_ROOT}`), 'utf8'))
    }
    console.log(`Reset ${databaseName} with schema.sql and ${FIXTURES.join(', ')}`)
  } finally {
    await client.end()
  }
}

await ensureDatabase()
await resetSchema()
