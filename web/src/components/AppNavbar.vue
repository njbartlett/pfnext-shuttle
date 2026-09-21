<template>
  <nav class="navbar navbar-expand-lg bg-body-tertiary" role="navigation">
    <div class="container-fluid">
      <!-- Brand -->
      <RouterLink class="navbar-brand" to="/index.html">
        <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30" fill="currentColor" class="bi bi-sun-fill" viewBox="0 0 54 54">
          <path d="m 29.64,32.02 2.7,-4.68 8.16,14.04 h 5.4 C 44.7,39.46 33.3,19.96 32.34,18.04 L 20.58,38.44 16.2,46.06 H 0 C 0.9,44.56 6.96,34 12.78,23.86 16.2,17.92 19.98,11.26 24.24,4 h 5.4 L 18.48,23.32 8.1,41.38 h 5.4 C 20.88,28.36 25.5,20.68 32.34,8.68 32.76,9.4 53.88,46 54,46.06 H 37.8 Z"/>
        </svg>
        {{ SITE_NAME }}
      </RouterLink>

      <!-- Collapse state toggle button -->
      <button class="navbar-toggler" type="button" data-bs-toggle="collapse" data-bs-target="#navbarContent" aria-controls="navbarContent" aria-expanded="false" aria-label="Toggle navigation">
        <span class="navbar-toggler-icon"></span>
      </button>

      <!-- Navbar collapsible list -->
      <div id="navbarContent" ref="collapseElement" class="collapse navbar-collapse">
        <ul class="navbar-nav me-auto">
          <!-- General pages, from the router -->
          <li v-for="item in navigationRoutes" :key="item.path" class="nav-item">
            <RouterLink class="nav-link" active-class="active" :to="item.path">{{ item.meta?.title }}</RouterLink>
          </li>

          <!-- Admin only links -->
          <li v-if="isAdmin" class="nav-item dropdown">
            <a class="nav-link dropdown-toggle" href="#" role="button" data-bs-toggle="dropdown" aria-expanded="false"><i class="bi bi-gear-fill"></i></a>
            <ul class="dropdown-menu">
              <li><RouterLink class="dropdown-item" to="/edit_session.html"><i class="bi bi-plus-circle"></i>&nbsp;Create Sessions</RouterLink></li>
              <li><RouterLink class="dropdown-item" to="/bulk_sessions.html"><i class="bi bi-list-ol"></i>&nbsp;Create Multiple Sessions</RouterLink></li>
              <li><RouterLink class="dropdown-item" to="/polls_admin.html"><i class="bi bi-card-checklist"></i>&nbsp;Manage Polls</RouterLink></li>
              <li><RouterLink class="dropdown-item" to="/members.html"><i class="bi bi-people-fill"></i>&nbsp;Members</RouterLink></li>
              <li><RouterLink class="dropdown-item" to="/loginlist.html"><i class="bi bi-person-lines-fill"></i>&nbsp;Logged-in Users</RouterLink></li>
              <li><RouterLink class="dropdown-item" to="/sessions_report.html"><i class="bi bi-file-earmark-text"></i>&nbsp;Sessions Report</RouterLink></li>
              <li><RouterLink class="dropdown-item" to="/stats.html"><i class="bi bi-bar-chart-fill"></i>&nbsp;Attendance Stats</RouterLink></li>
              <li><RouterLink class="dropdown-item" to="/logs.html"><i class="bi bi-journal-text"></i>&nbsp;Logs</RouterLink></li>
              <li><RouterLink class="dropdown-item" :to="{ name: 'edit_post', params: { location: 'new' } }"><i class="bi bi-feather"></i>&nbsp;Create Blog Post</RouterLink></li>
            </ul>
          </li>
          <li class="nav-item"><a class="nav-link" href="https://chat.whatsapp.com/CPjp84Qz6uM5t9Oq88Qb4Q" target="_blank" title="WhatsApp Help Channel"><i class="bi bi-question-circle"></i>&nbsp;Help</a></li>
          <li class="nav-item"><a class="nav-link" href="https://www.instagram.com/anotherlevelcommunityfitness" target="_blank" title="Instagram"><i class="bi bi-instagram"></i></a></li>
        </ul>

        <!-- User account links if logged in -->
        <div v-if="user">
          <a href="#" role="button" class="nav-link dropdown-toggle" data-bs-toggle="dropdown" aria-expanded="false">
            <span class="navbar-text">
              <img v-if="user.status === 'green'" class="icon" src="/img/bib-green.svg" title="Green Bib Member">
              <img v-else-if="user.status === 'red'" class="icon" src="/img/bib-red.svg" title="Red Bib Member">
              <img v-else-if="user.status === 'blue'" class="icon" src="/img/bib-blue.svg" title="Blue Bib Member">
              <i v-else class="bi bi-person-fill"></i>
              &nbsp;{{ user.name }}
            </span>
          </a>
          <ul class="dropdown-menu dropdown-menu-lg-end">
            <li><RouterLink to="/bookings.html" class="dropdown-item">Bookings</RouterLink></li>
            <li><RouterLink to="/profile.html" class="dropdown-item">My Profile</RouterLink></li>
            <li><hr class="dropdown-divider"></li>
            <li><button type="button" id="logout" class="dropdown-item text-danger" @click="onLogout"><i class="bi bi-box-arrow-right"></i> Logout</button></li>
          </ul>
        </div>

        <!-- Login/Register if not logged in -->
        <div v-else class="btn-group" role="group">
          <RouterLink id="navbarRegister" class="btn btn-outline-success rounded-pill-left" to="/register.html">Register</RouterLink>
          <RouterLink id="navbarLogin" class="btn btn-outline-success rounded-pill-right" :to="loginRoute(route.fullPath)">Login</RouterLink>
        </div>

        <ThemeToggle />
      </div>
    </div>
  </nav>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { Collapse } from 'bootstrap'
import { useRoute, useRouter } from 'vue-router'
import { SITE_NAME } from '@/app/site'
import ThemeToggle from './ThemeToggle.vue'
import { loginRoute, navigationRoutes } from '@/router'
import { isAdmin, logout, user } from '@/stores/auth'

const route = useRoute()
const router = useRouter()
const collapseElement = ref<HTMLElement | null>(null)

async function onLogout() {
  // Leave member-only pages before the login state clears, so the router
  // does not bounce through the login page on the way home
  if (!route.meta.stayOnLogout) {
    await router.push('/index.html')
  }
  await logout()
}

// On small screens the expanded menu would otherwise stay open across
// in-app navigation, where a page load used to close it
watch(
  () => route.fullPath,
  () => {
    if (collapseElement.value?.classList.contains('show')) {
      Collapse.getOrCreateInstance(collapseElement.value).hide()
    }
  }
)
</script>
