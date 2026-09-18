# Another Level — website front-end

Vite + Vue 3 + TypeScript build for the website pages, migrating them one at
a time away from the Tera-rendered, global-script pages under `templates/`
and `static/js/`. Shares its API client, types and booking service with the
mobile app through `packages/shared` (`@pfnext/shared`).

## How a page is served

1. `templates/pages.toml` marks a page `module = true`.
2. `src/templates.rs` then renders `templates/module_page.html.tera` for it
   instead of a page-specific template. That layout embeds the Tera `common`
   and `page` context as JSON and loads `/js/pages/<template_name>.js` as an
   ES module.
3. `web/src/pages/<template_name>.ts` is the matching Vite entry. It calls
   `mountPage(SomeView)`, which mounts `PageShell` (navbar + API error banner
   + the view) on `#app`.

Every page in `pages.toml` is now a module page. The only legacy (Tera plus
global script) pages left are the blog index, blog post and blog editor under
`/blog`, which `src/blog.rs` renders with its own context; they still use the
trimmed `static/js/library.js`.

## Building

From the repo root:

```sh
npm install            # installs web/ and packages/shared (npm workspaces)
npm run build          # type-checks, then writes static/js/pages/
npm run watch          # rebuild on change while running the Rocket server
```

`static/js/pages/` is build output and is git-ignored; the Dockerfile builds
it in a Node stage. Run `npm run build` before `cargo run` locally or module
pages will 404 their script.

The mobile app is deliberately not part of the workspace: Capacitor's native
projects hard-code `mobile/node_modules/...` paths, so it links
`packages/shared` as a `file:` dependency and keeps its own `node_modules`.

## Layout

- `src/pages/` — one entry per module page; nothing but `mountPage(View, options)`.
  Options: `navbar: false` for the focused editor/auth pages, `stayOnLogout`
  for pages that work logged out.
- `src/views/` — page components
- `src/components/` — `PageShell`, `AppNavbar`, `ApiErrorAlert`, `RequireLogin`
  (member-only gate), and reusable UI: `PagerBar`, `MemberPicker`,
  `BsModal`/`ConfirmModal`, `SessionControls`, `SessionRatings`, `StarRating`,
  `ChallengeProgressBars`, `BibIcon`, `AdminPanel`, `SortButtons`
- `src/composables/` — `usePagedWindow` (week/month paging), `useNow` (1 s
  clock), `useHashState`/`urlQuery` (URL state; becomes route queries in the
  SPA step), `useReturnPath` (`?return=`), `useDirtyTracking`,
  `loadSelectableMembers`, `rankByScore`, date-input helpers
- `src/stores/` — `auth` (logged-in user, mirrored in localStorage under the
  same key as `library.js`; `login`, `logout`, role flags), `apiError`
  (`tryApi()` runs a call and shows any failure in the banner)
- `src/app/` — `mountPage`, page context reader, API client configuration

API calls never happen in views directly: they go through the typed service
modules in `packages/shared/src` (`users`, `sessions`, `bookingService`,
`polls`, `activities`, `admin`).

## Current constraints

- Bootstrap, Popper and `theme.js` are still loaded as classic scripts by
  `base_layout.html.tera`, so components use the `bootstrap` global
  (`src/globals.d.ts`) rather than importing it.
- SFC `<style>` blocks are emitted as separate CSS files that the Tera layout
  does not link. Use Bootstrap utility classes or inline styles until the
  single-entry SPA step, when Vite owns the HTML.
