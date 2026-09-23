# Another Level — website front-end

Vite + Vue 3 + TypeScript single-page app for the website. Shares its API
client, types and services with the mobile app through `packages/shared`
(`@pfnext/shared`).

## How the site is served

- `npm run build` writes the app to `web/dist/` (Vite-owned `index.html` plus
  hashed assets, with the files in `public/` copied as-is and TinyMCE's skins
  copied to `tinymce/skins/`).
- Rocket (`src/templates.rs`) serves files from `web/dist/` and returns
  `web/dist/index.html` for any page path with no file (an SPA fallback).
  Paths under `/api`, and missing assets, 404 instead.
- `src/router/index.ts` owns the page URLs. They keep their historical
  `.html` form (`/sessions.html`) so bookmarks and emailed links still work,
  and the old fragment state (`sessions.html#week=...`) is redirected to
  query parameters.

The blog is part of the app: `/blog/index.html`, `/blog/posts/<title>.html`
and `/blog/edit/<title>.html` are routes backed by the posts API
(`src/blog.rs`), and images embedded in posts keep their `/blog/blobs/<id>`
URLs. For post pages Rocket serves the shell with the post's Open Graph tags
added (`src/blog.rs`, `post_page`), so links shared in chat and on social
media get a title, description and image. The editor is TinyMCE from npm
(`tinymce`, `@tinymce/tinymce-vue`); `vite.config.ts` copies its skins into
the build because the editor loads them by URL. Writing posts needs the
`admin` or `editor` role.

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

The dev server proxies `/api` and `/blog/blobs` to Rocket on
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
  icon font and JavaScript from npm, and the site styles in `src/styles/`
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
  `useReturnPath` (`?return=`), `useTheme`, `usePagedWindow` (day/week/month
  paging), `useMediaQuery` (a reactive `matchMedia`, used by the sessions
  calendar to show one day at a time below the `md` breakpoint), `useNow`
  (1 s clock), `useDirtyTracking`, `loadSelectableMembers`, `rankByScore`,
  date-input helpers
- `src/stores/` — `auth` (logged-in user, mirrored in localStorage; `login`,
  `logout`, role flags including `isEditor`), `apiError` (`tryApi()` runs a
  call and shows any failure in the banner)
- `src/app/site.ts` — site name
- `src/testing/` — component test support (see Testing)

API calls never happen in views directly: they go through the typed service
modules in `packages/shared/src` (`users`, `sessions`, `bookingService`,
`polls`, `posts`, `activities`, `admin`).

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
`web/playwright.config.ts`. They cover what the mocked layers cannot: the
API contract between the pages and Rocket. Playwright starts Rocket itself on
port 8010 with `web/dist` freshly built, pointed at a dedicated `pfnext_test`
database on the Postgres server named by `DATABASE_URL` in the repo's `.env`
(override with `E2E_DATABASE_URL`). `e2e/reset-db.mjs` recreates that database
from `schema.sql` and the SQL files in `src/fixtures/` before every run, and
refuses to touch a database whose name does not end in `_test`. The fixture
accounts (`admin@example.com`, `trainer@example.com`, `user1@example.com`,
password `password`) are the ones the Rust tests use. Rocket's other required
settings come from `.env`, or from test-only defaults in `e2e/env.ts` where
there is no `.env` (CI). The browser runs as `Europe/London`, `en-GB`.

Specs import `test` and `expect` from `e2e/fixtures.ts`, whose `page` aborts
every request to a host other than localhost: the Instagram embed and the
carousel images would otherwise make page loads depend on the network.
`e2e/api.ts` logs in to the API directly to arrange data (creating the session
a spec books) and to check the server afterwards. `auth.setup.ts` logs in once
per role through the real login page and saves each browser state to
`e2e/.auth/`, so specs start authenticated as the member, trainer or admin.

- `logged-out.spec.ts` sweeps every page URL, the login redirects, the legacy
  fragment links from emails and bookmarks, and Rocket's SPA fallback.
- `booking.spec.ts` books and cancels a session as the member, on the sessions
  page and again from the bookings page.
- `attendance.spec.ts` opens the attendance tool from a session as its trainer,
  marks the booked member present, clears and re-marks everyone, and returns to
  the week it came from.
- `admin-sessions.spec.ts` creates a session in the editor as the admin, copies
  it from the sessions page and deletes both.
- `blog.spec.ts` writes and publishes a post in TinyMCE as the admin, reads it
  back with its Open Graph tags, and deletes it.

Run `npx playwright show-trace web/test-results/<test>/trace.zip` to inspect a
failure. For faster iteration, run Rocket yourself against the test database
and the Vite dev server (`npm run dev`), then point Playwright at it:

```sh
DATABASE_URL=postgres://.../pfnext_test node web/e2e/reset-db.mjs
DATABASE_URL=postgres://.../pfnext_test COOKIE_SECURE=false cargo run   # one terminal
npm run dev                                                             # another
E2E_BASE_URL=http://localhost:5173 npm run test:e2e
```

With `E2E_BASE_URL` set Playwright starts and resets nothing. Set
`E2E_SKIP_WEB_BUILD=1` to have Playwright start Rocket without rebuilding
`web/dist` when it is already built, for instance by extracting it from the
Dockerfile's `web-builder` stage
(`docker build --target web-builder --output type=local,dest=out .`, then
`out/web/dist`).

**Continuous integration** (`.github/workflows/ci.yml`) runs `cargo test` and
the Vitest suites on every push, each with a Postgres service where needed,
and the Playwright suite on pull requests, building `web/dist` and the Rocket
binary once before Playwright starts the server. Traces of failed specs are
uploaded as a workflow artifact.
