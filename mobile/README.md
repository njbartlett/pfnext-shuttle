# Another Level — mobile app

Capacitor + Ionic Vue app for the pfnext booking system. It is a second client
of the same Rocket backend, talking to the versioned API at `/api/v1` with a
bearer token (see Phase 0: `POST /login` with `"client": "mobile"`).

## Prerequisites

- Node 20+
- Android: Android Studio (for emulator/device builds)
- iOS: Xcode and CocoaPods (`brew install cocoapods`), then `npx cap add ios`

## Development

```sh
npm install
cp .env.example .env      # points the app at http://localhost:8000/api/v1
npm run dev               # browser dev server; run the Rocket backend alongside
```

The backend's `CORS_ALLOWED` (in the repo root `.env`) already allows any
localhost port plus the `capacitor://localhost` / `https://localhost` app
origins.

To test on a device or emulator against a local backend, set
`VITE_API_BASE_URL` to the machine's LAN address. With no `.env` at all, the
app talks to production (`https://anotherlevelfitness.uk/api/v1`).

## Native builds

```sh
npm run sync              # type-check, vite build, copy into native projects
npx cap open android      # opens Android Studio
npx cap open ios          # opens Xcode (after `npx cap add ios`)
```

## Troubleshooting

**CocoaPods fails with `Could not find 'base64' … among N total gem(s)`**:
Homebrew's CocoaPods runs under Homebrew Ruby 4, but a version manager
(chruby/rbenv) exporting `GEM_PATH`/`GEM_HOME` overrides Ruby 4's own gem
paths, hiding its bundled gems. Run pod-invoking commands with those unset —
the `npm run sync` script already does this:

```sh
env -u GEM_PATH -u GEM_HOME npx cap sync ios
```

## Structure

The API client, response types, booking service and date formatting live in
the repo-level `packages/shared` package (`@pfnext/shared`), which is also
used by the website build in `web/`. It is linked here as a `file:`
dependency rather than an npm workspace because the Capacitor native projects
hard-code `mobile/node_modules/...` paths and would break under hoisting.

- `@pfnext/shared` `api.ts` — fetch wrapper: base URL, bearer token,
  `{code, message}` error parsing; configured in `src/main.ts`
- `src/lib/auth.ts` — login/logout/restore; session persisted via Keychain/Keystore
  (`src/lib/tokenStorage.ts`)
- `src/lib/bookingService.ts` — binds the shared sessions, bookings and
  waitlist calls to the logged-in user; the `credits_opt_in_required` (402)
  handshake surfaces as a `credits_required` result that the UI confirms with
  the user before re-booking with `credits_used` set
- `src/views/` — Login, Sessions list, Session detail (book / cancel /
  waitlist), My Bookings, Profile
