# Another Level — website front-end

Vite + Vue 3 + TypeScript single-page app for the website. Shares its API
client, types and services with the mobile app through `packages/shared`
(`@pfnext/shared`).

## How the site is served

- `npm run build` writes the app to `web/dist/` (Vite-owned `index.html` plus
  hashed assets, with the files in `public/` copied as-is).
- Rocket (`src/templates.rs`) serves files from `web/dist/` first, then from
  `static/`, and returns `web/dist/index.html` for any page path with no file
  (an SPA fallback). Paths under `/api` and `/blog`, and missing assets, 404
  instead.
- `src/router/index.ts` owns the page URLs. They keep their historical
  `.html` form (`/sessions.html`) so bookmarks and emailed links still work,
  and the old fragment state (`sessions.html#week=...`) is redirected to
  query parameters.

The blog (`/blog/...`) is still rendered by Rocket from Tera templates with
the legacy global scripts in `static/js/`; see the migration plan for the
options.

## Building and developing

From the repo root:

```sh
npm install            # installs web/ and packages/shared (npm workspaces)
npm run build          # type-checks, then writes web/dist/
npm run dev            # Vite dev server with hot reload on :5173
npm run watch          # rebuild web/dist/ on change instead
```

`web/dist/` is build output and is git-ignored; the Dockerfile builds it in a
Node stage. Run `npm run build` before `cargo run` locally or Rocket has no
pages to serve.

The dev server proxies `/api`, `/blog`, `/js` and `/styles` to Rocket on
`localhost:8000`, so `cargo run` must be running alongside it. The browser
only talks to the dev server, so the session cookie is same-site; with
`COOKIE_SECURE=true` it still works because browsers treat `localhost` as a
secure context.

The mobile app is deliberately not part of the workspace: Capacitor's native
projects hard-code `mobile/node_modules/...` paths, so it links
`packages/shared` as a `file:` dependency and keeps its own `node_modules`.

## Layout

- `index.html` — the SPA shell; applies the saved colour theme before first
  paint
- `public/` — images, icons and the web manifest, copied to `dist/` unchanged
- `src/main.ts` — creates the app with the router; imports Bootstrap's CSS,
  icon font and JavaScript from npm
- `src/App.vue` — navbar, API error banner, `<RouterView>`, footer; sends a
  member page back to the login page when the login expires
- `src/router/` — one route per page with `meta` (`title`, `nav`, `navbar`,
  `requiresLogin`, `stayOnLogout`); `createAppRouter()` attaches the login
  guard and the legacy fragment redirect; `loginRoute()` and
  `passwordResetUrl()` helpers
- `src/views/` — page components, lazy-loaded by the router
- `src/components/` — `AppNavbar`, `ThemeToggle`, `ApiErrorAlert`, and
  reusable UI: `PagerBar`, `MemberPicker`, `BsModal`/`ConfirmModal`,
  `SessionControls`, `SessionRatings`, `StarRating`, `ChallengeProgressBars`,
  `BibIcon`, `AdminPanel`, `AuthCard`, `PageTitle`, `SortButtons`
- `src/composables/` — `useQueryState` (page state in the route query),
  `useReturnPath` (`?return=`), `useTheme`, `usePagedWindow` (week/month
  paging), `useNow` (1 s clock), `useDirtyTracking`, `loadSelectableMembers`,
  `rankByScore`, date-input helpers
- `src/stores/` — `auth` (logged-in user, mirrored in localStorage under the
  same key as `static/js/library.js`; `login`, `logout`, role flags),
  `apiError` (`tryApi()` runs a call and shows any failure in the banner)
- `src/app/site.ts` — site name and blog URLs
- `src/testing/` — component test support (see Testing)

API calls never happen in views directly: they go through the typed service
modules in `packages/shared/src` (`users`, `sessions`, `bookingService`,
`polls`, `activities`, `admin`).

Bootstrap components are imported from `bootstrap` where a view needs one
(`Modal`, `Carousel`, `Collapse`); the data-api for dropdowns and collapses
is enabled by the `import 'bootstrap'` in `main.ts`.

## Testing

```sh
npm test               # Vitest unit tests in packages/shared and web
npm run test:e2e       # Playwright end-to-end tests against the real stack
```

**Unit tests** (`*.test.ts` next to the code, run with `TZ=UTC` under jsdom)
cover the pure logic: the API client, the booking credits handshake, date
formatting and calendar arithmetic in `packages/shared`, and the composables
in `web/src/composables`. Add tests beside the module they exercise.

**Component tests** (also `*.test.ts`, with Vue Test Utils) mount views and
components with the logged-in state that the end-to-end sweep cannot reach
cheaply, and never talk to Rocket. The pieces, all in `src/testing/`:

- `setup.ts` runs before every web test file: it fills jsdom's gaps
  (`matchMedia`, `scrollTo`) and replaces `fetch` with a function that
  rejects, so an unmocked call fails loudly.
- `sharedMock.ts` is the mocked `@pfnext/shared`. A test file opts in with
  `vi.mock('@pfnext/shared', () => import('@/testing/sharedMock').then((m) => m.mockedShared()))`;
  every async export (all the API calls) becomes a `vi.fn()` that rejects until
  the test gives it a value with `vi.mocked(listSessions).mockResolvedValue(...)`,
  while the pure helpers and `ApiError` stay real.
- `mount.ts` provides `mountAt(component, path)`, which mounts on a fresh copy
  of the app's router (guards included) over an in-memory history, and
  `waitFor()` for Bootstrap transitions and lazy-loaded routes.
- `fixtures.ts` holds the users, a session and a pinned clock (`NOW`, a
  Wednesday in June 2025, applied with `vi.setSystemTime`).

Logged-in state is just `setUser(MEMBER)` on the auth store. The current
tests cover `SessionControls` per role and deadline, the sessions page's
booking flow including the credits confirmation, the editor filling its form
from `?edit=` and `?copy=`, and, through the real `App` shell, the login guard
and its return link, a login that expires on a member page, logging out and
the legacy fragment links. Components that open Bootstrap modals need
`attachTo: document.body`, and a test must wait for `shown.bs.modal` before
dismissing a modal because Bootstrap ignores `hide()` mid-transition.

**End-to-end tests** live in `web/e2e/` and are configured by
`web/playwright.config.ts`. Playwright starts Rocket itself on port 8010 with
`web/dist` freshly built, pointed at a dedicated `pfnext_test` database on the
Postgres server named by `DATABASE_URL` in the repo's `.env` (override with
`E2E_DATABASE_URL`). `e2e/reset-db.mjs` recreates that database from
`schema.sql` and `src/fixtures/users.sql` before every run and refuses to touch
a database whose name does not end in `_test`. The fixture accounts
(`admin@example.com`, `trainer@example.com`, `user1@example.com`, password
`password`) are the ones the Rust tests use.

Specs import `test` and `expect` from `e2e/fixtures.ts`, whose `page` aborts
every request to a host other than localhost: the Instagram embed and the
carousel images would otherwise make page loads depend on the network.
`auth.setup.ts` logs in as a member through the real login page and saves the
browser state to `e2e/.auth/`, so specs start authenticated. `logged-out.spec.ts`
sweeps every page URL, the login redirects, the legacy fragment links and
Rocket's SPA fallback. `booking.spec.ts` creates a session through the API as
the admin, then books and cancels it as the member. Run
`npx playwright show-trace web/test-results/<test>/trace.zip` to inspect a
failure.

## Still shared with the blog

`static/styles/al.css` is linked by `index.html` rather than imported, because
the Tera blog pages load the same file. The colour theme is stored under the
same localStorage key that `static/js/theme.js` reads. Both move into `src/`
once the blog leaves Tera.
