import { createRouter, createWebHistory } from '@ionic/vue-router'
import type { RouteRecordRaw } from 'vue-router'
import { isAuthenticated, restoreSession } from '@/lib/auth'
import TabsPage from '@/views/TabsPage.vue'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    redirect: '/tabs/sessions'
  },
  {
    path: '/login',
    component: () => import('@/views/LoginPage.vue')
  },
  {
    path: '/tabs/',
    component: TabsPage,
    children: [
      {
        path: '',
        redirect: '/tabs/sessions'
      },
      {
        path: 'sessions',
        component: () => import('@/views/SessionsPage.vue')
      },
      {
        path: 'sessions/:id',
        component: () => import('@/views/SessionDetailPage.vue')
      },
      {
        path: 'bookings',
        component: () => import('@/views/BookingsPage.vue')
      },
      {
        path: 'profile',
        component: () => import('@/views/ProfilePage.vue')
      }
    ]
  }
]

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes
})

router.beforeEach(async (to) => {
  await restoreSession()
  if (to.path !== '/login' && !isAuthenticated.value) {
    return { path: '/login', query: { return: to.fullPath } }
  }
  if (to.path === '/login' && isAuthenticated.value) {
    return { path: '/tabs/sessions' }
  }
  return true
})

export default router
