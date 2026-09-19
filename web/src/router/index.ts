// One route per page. Paths keep their historical ".html" form so that
// bookmarks, emails and the mobile app's links continue to resolve; Rocket
// serves web/dist/index.html for every page path (src/templates.rs).
import { watch } from 'vue'
import {
  createRouter, createWebHistory, type RouteLocationNormalized, type RouteLocationRaw, type RouteRecordRaw
} from 'vue-router'
import { SITE_NAME } from '@/app/site'
import HomeView from '@/views/HomeView.vue'
import { isLoggedIn } from '@/stores/auth'

declare module 'vue-router' {
  interface RouteMeta {
    // Page heading and document title
    title: string
    // Listed in the navbar
    nav?: boolean
    // false for focused pages (login, editors) rendered without the navbar
    navbar?: boolean
    // Redirect to the login page (with a return link) when logged out
    requiresLogin?: boolean
    // Logging out keeps the user on this page instead of going home
    stayOnLogout?: boolean
  }
}

export const routes: RouteRecordRaw[] = [
  { path: '/', redirect: '/index.html' },
  { path: '/index.html', name: 'home', component: HomeView, meta: { title: 'Home', nav: true } },
  { path: '/about.html', name: 'about', component: () => import('@/views/AboutView.vue'), meta: { title: 'About', nav: true } },
  { path: '/pricing.html', name: 'pricing', component: () => import('@/views/PricingView.vue'), meta: { title: 'Pricing', nav: true } },
  {
    path: '/sessions.html',
    name: 'sessions',
    component: () => import('@/views/SessionsView.vue'),
    meta: { title: 'Sessions', nav: true, stayOnLogout: true }
  },
  {
    path: '/challenges.html',
    name: 'challenges',
    component: () => import('@/views/ChallengesView.vue'),
    meta: { title: 'Challenges', nav: true, requiresLogin: true }
  },
  {
    path: '/activities.html',
    name: 'activities',
    component: () => import('@/views/ActivitiesView.vue'),
    meta: { title: 'Activities', nav: true, requiresLogin: true }
  },
  { path: '/polls.html', name: 'polls', component: () => import('@/views/PollsView.vue'), meta: { title: 'Polls', nav: true, requiresLogin: true } },
  { path: '/bookings.html', name: 'bookings', component: () => import('@/views/BookingsView.vue'), meta: { title: 'Bookings', requiresLogin: true } },
  { path: '/profile.html', name: 'profile', component: () => import('@/views/ProfileView.vue'), meta: { title: 'Profile', requiresLogin: true } },

  // Authentication flows: no navbar
  { path: '/login.html', name: 'login', component: () => import('@/views/LoginView.vue'), meta: { title: 'Login', navbar: false, stayOnLogout: true } },
  { path: '/register.html', name: 'register', component: () => import('@/views/RegisterView.vue'), meta: { title: 'Register', navbar: false } },
  {
    path: '/passwordreset.html',
    name: 'passwordreset',
    component: () => import('@/views/PasswordResetView.vue'),
    meta: { title: 'Password Reset', navbar: false }
  },

  // Trainer and admin tools opened from a session, with a ?return= link back
  {
    path: '/attendance.html',
    name: 'attendance',
    component: () => import('@/views/AttendanceView.vue'),
    meta: { title: 'Session Attendance', navbar: false, requiresLogin: true }
  },
  {
    path: '/feedback.html',
    name: 'feedback',
    component: () => import('@/views/FeedbackView.vue'),
    meta: { title: 'Session Feedback', navbar: false, requiresLogin: true }
  },
  {
    path: '/edit_session.html',
    name: 'edit_session',
    component: () => import('@/views/EditSessionView.vue'),
    meta: { title: 'Create/Edit Session', navbar: false, requiresLogin: true }
  },
  {
    path: '/bulk_sessions.html',
    name: 'bulk_sessions',
    component: () => import('@/views/BulkSessionsView.vue'),
    meta: { title: 'Create Multiple Sessions', navbar: false, requiresLogin: true }
  },

  // Admin pages, reached from the navbar's admin menu
  { path: '/polls_admin.html', name: 'polls_admin', component: () => import('@/views/PollsAdminView.vue'), meta: { title: 'Manage Polls', requiresLogin: true } },
  { path: '/members.html', name: 'members', component: () => import('@/views/MembersView.vue'), meta: { title: 'Member Directory', requiresLogin: true } },
  { path: '/logs.html', name: 'logs', component: () => import('@/views/LogsView.vue'), meta: { title: 'Logs', requiresLogin: true } },
  { path: '/loginlist.html', name: 'loginlist', component: () => import('@/views/LoginListView.vue'), meta: { title: 'Login List', requiresLogin: true } },
  { path: '/stats.html', name: 'stats', component: () => import('@/views/StatsView.vue'), meta: { title: 'Attendance Stats', requiresLogin: true } },
  {
    path: '/sessions_report.html',
    name: 'sessions_report',
    component: () => import('@/views/SessionsReportView.vue'),
    meta: { title: 'Sessions Report', requiresLogin: true }
  },

  { path: '/:pathMatch(.*)*', name: 'not-found', component: () => import('@/views/NotFoundView.vue'), meta: { title: 'Not Found' } }
]

// Navbar entries, in definition order
export const navigationRoutes = routes.filter((route) => route.meta?.nav)

export const router = createRouter({
  history: createWebHistory(),
  routes,
  scrollBehavior(to, _from, savedPosition) {
    if (savedPosition) {
      return savedPosition
    }
    if (to.hash) {
      return { el: to.hash }
    }
    return { top: 0 }
  }
})

// The login page, returning to `returnPath` afterwards
export function loginRoute(returnPath: string): RouteLocationRaw {
  return { name: 'login', query: { return: returnPath } }
}

// Absolute URL of a route, for links sent by email
export function absoluteUrl(to: RouteLocationRaw): string {
  return new URL(router.resolve(to).href, window.location.origin).href
}

export function passwordResetUrl(): string {
  return absoluteUrl({ name: 'passwordreset' })
}

// Before the single-page app, pages kept their state in the URL fragment
// (sessions.html#week=..., edit_session.html#edit=...). Such links still
// arrive from emails and bookmarks; carry the values over into the query.
function legacyFragmentRedirect(to: RouteLocationNormalized): RouteLocationRaw | undefined {
  if (!to.hash.includes('=')) {
    return undefined
  }
  const query = { ...to.query }
  new URLSearchParams(to.hash.substring(1)).forEach((value, key) => {
    query[key] = value
  })
  return { path: to.path, query, hash: '' }
}

router.beforeEach((to) => {
  const redirect = legacyFragmentRedirect(to)
  if (redirect) {
    return redirect
  }
  if (to.meta.requiresLogin && !isLoggedIn.value) {
    return loginRoute(to.fullPath)
  }
  return true
})

router.afterEach((to) => {
  document.title = `${to.meta.title} – ${SITE_NAME}`
})

// A login that expires or is revoked while on a member page (the API answered
// 401 and the auth store cleared the user) sends the user to log in again
watch(isLoggedIn, (loggedIn) => {
  const current = router.currentRoute.value
  if (!loggedIn && current.meta.requiresLogin) {
    void router.replace(loginRoute(current.fullPath))
  }
})
