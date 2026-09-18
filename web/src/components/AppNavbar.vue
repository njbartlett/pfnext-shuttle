<template>
  <nav class="navbar navbar-expand-lg bg-body-tertiary" role="navigation">
    <div class="container-fluid">
      <!-- Brand -->
      <a class="navbar-brand">
        <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30" fill="currentColor" class="bi bi-sun-fill" viewBox="0 0 54 54">
          <path d="m 29.64,32.02 2.7,-4.68 8.16,14.04 h 5.4 C 44.7,39.46 33.3,19.96 32.34,18.04 L 20.58,38.44 16.2,46.06 H 0 C 0.9,44.56 6.96,34 12.78,23.86 16.2,17.92 19.98,11.26 24.24,4 h 5.4 L 18.48,23.32 8.1,41.38 h 5.4 C 20.88,28.36 25.5,20.68 32.34,8.68 32.76,9.4 53.88,46 54,46.06 H 37.8 Z"/>
        </svg>
        {{ common.branding }}
      </a>

      <!-- Collapse state toggle button -->
      <button class="navbar-toggler" type="button" data-bs-toggle="collapse" data-bs-target="#navbarContent" aria-controls="navbarContent" aria-expanded="false" aria-label="Toggle navigation">
        <span class="navbar-toggler-icon"></span>
      </button>

      <!-- Navbar collapsible list -->
      <div id="navbarContent" class="collapse navbar-collapse">
        <ul class="navbar-nav me-auto">
          <!-- General pages, from templates/pages.toml -->
          <li v-for="item in common.navigation" :key="item.url" class="nav-item">
            <a v-if="isCurrent(item.url)" class="nav-link active" aria-current="page">{{ item.title }}</a>
            <a v-else class="nav-link" :href="item.url">{{ item.title }}</a>
          </li>

          <!-- Blog posts are handled a bit differently -->
          <li class="nav-item">
            <a v-if="isCurrent(BLOG_URL)" class="nav-link active" aria-current="page">Posts</a>
            <a v-else class="nav-link" :href="BLOG_URL">Posts</a>
          </li>

          <!-- Admin only links -->
          <li v-if="isAdmin" class="nav-item dropdown">
            <a class="nav-link dropdown-toggle" href="#" role="button" data-bs-toggle="dropdown" aria-expanded="false"><i class="bi bi-gear-fill"></i></a>
            <ul class="dropdown-menu">
              <li><a class="dropdown-item" href="/edit_session.html"><i class="bi bi-plus-circle"></i>&nbsp;Create Sessions</a></li>
              <li><a class="dropdown-item" href="/bulk_sessions.html"><i class="bi bi-list-ol"></i>&nbsp;Create Multiple Sessions</a></li>
              <li><a class="dropdown-item" href="/polls_admin.html"><i class="bi bi-card-checklist"></i>&nbsp;Manage Polls</a></li>
              <li><a class="dropdown-item" href="/members.html"><i class="bi bi-people-fill"></i>&nbsp;Members</a></li>
              <li><a class="dropdown-item" href="/loginlist.html"><i class="bi bi-person-lines-fill"></i>&nbsp;Logged-in Users</a></li>
              <li><a class="dropdown-item" href="/sessions_report.html"><i class="bi bi-file-earmark-text"></i>&nbsp;Sessions Report</a></li>
              <li><a class="dropdown-item" href="/stats.html"><i class="bi bi-bar-chart-fill"></i>&nbsp;Attendance Stats</a></li>
              <li><a class="dropdown-item" href="/logs.html"><i class="bi bi-journal-text"></i>&nbsp;Logs</a></li>
              <li><a class="dropdown-item" href="/blog/edit/new"><i class="bi bi-feather"></i>&nbsp;Create Blog Post</a></li>
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
            <li><a href="/bookings.html" class="dropdown-item">Bookings</a></li>
            <li><a href="/profile.html" class="dropdown-item">My Profile</a></li>
            <li><hr class="dropdown-divider"></li>
            <li><button type="button" id="logout" class="dropdown-item text-danger" @click="logout()"><i class="bi bi-box-arrow-right"></i> Logout</button></li>
          </ul>
        </div>

        <!-- Login/Register if not logged in -->
        <div v-else class="btn-group" role="group">
          <a id="navbarRegister" class="btn btn-outline-success rounded-pill-left" href="/register.html">Register</a>
          <a id="navbarLogin" class="btn btn-outline-success rounded-pill-right" :href="'/login.html?return=' + encodeURIComponent(common.url)">Login</a>
        </div>

        <!-- Theme selector. Driven by static/js/theme.js, which binds these
             elements on DOMContentLoaded; the module script that mounts this
             component runs before that event, so the elements exist in time. -->
        <a id="bd-theme" href="#" role="button" class="nav-link dropdown-toggle navbar-text px-0 px-lg-2" data-bs-toggle="dropdown" aria-expanded="false">
          <i class="bi bi-moon-fill" id="bd-theme-icon"></i><span class="d-lg-none ms-2" id="bd-theme-text">Toggle Theme</span>
        </a>
        <ul class="dropdown-menu dropdown-menu-lg-end">
          <li><a class="dropdown-item" data-bs-theme-value="dark"><i class="bi bi-moon-fill"></i>&nbsp;Dark</a></li>
          <li><a class="dropdown-item" data-bs-theme-value="light"><i class="bi bi-sun-fill"></i>&nbsp;Light</a></li>
        </ul>
      </div>
    </div>
  </nav>
</template>

<script setup lang="ts">
import { usePageContext } from '@/app/pageContext'
import { isAdmin, logout, user } from '@/stores/auth'

const BLOG_URL = '/blog/index.html'

const { common } = usePageContext()

function isCurrent(url: string): boolean {
  return common.url === url
}
</script>
