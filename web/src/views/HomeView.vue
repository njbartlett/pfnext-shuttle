<template>
  <div class="container">
    <!-- Banner -->
    <img src="/img/banner.svg" class="img-fluid" alt="Another Level Banner">
    <div class="text-center">
      <div v-if="!isLoggedIn" class="btn-group m-1 flex-nowrap">
        <a href="/register.html" class="btn btn-success rounded-pill-left" style="font-size: x-large;">Start&nbsp;Free&nbsp;Trial</a>
        <a href="/login.html?return=index.html" class="btn btn-outline-success rounded-pill-right" style="font-size: x-large;">Login</a>
      </div>
      <a href="/sessions.html" class="btn btn-outline-primary m-1" style="font-size: x-large;">Browse&nbsp;Sessions</a>
    </div>

    <!-- Slogan -->
    <div class="text-center mt-5 mx-1">
      <p class="fs-3 text-body-secondary">
        A fitness company focused on people not profit, community is key!
        We achieve results and forge bonds by being outdoors and supporting each other.
      </p>
    </div>

    <!-- Image carousel -->
    <div ref="carouselElement" class="carousel slide mt-5" data-bs-ride="carousel">
      <div class="carousel-inner">
        <div v-for="(image, index) in CAROUSEL_IMAGES" :key="image" class="carousel-item" :class="{ active: index === 0 }">
          <img :src="image" class="d-block w-100">
        </div>
      </div>
    </div>

    <div class="text-center my-5">
      <p class="fs-3 text-body-secondary">
        Ready to get started? Just <a href="/register.html">register a new account</a>, then book your first session for free!
      </p>
      <p class="fs-3 text-body-secondary">
        Read more <a href="/about.html">About Us</a> or view our <a href="/pricing.html">pricing plans</a>.
      </p>
    </div>
  </div>

  <div class="container-fluid py-3 bg-body-tertiary">
    <!-- Instagram feed -->
    <h2 class="display-4 fw-normal my-3 text-center">Latest Instagram Posts</h2>
    <div class="container ratio ratio-16x9">
      <iframe title="Another Level Fitness on Instagram" src="https://www.instagram.com/anotherlevelcommunityfitness/embed/" frameborder="0" allowfullscreen="false" style="padding: 0 12px 0 12px"></iframe>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { isLoggedIn } from '@/stores/auth'

const CAROUSEL_IMAGES = [
  'https://i.ibb.co/NgPjCngg/1.jpg',
  'https://i.ibb.co/Myn6nrjz/2.jpg',
  'https://i.ibb.co/JwvH4sxR/3.jpg',
  'https://i.ibb.co/ynt1wDwB/4.jpg',
  'https://i.ibb.co/Ndp8zMVB/5.jpg',
  'https://i.ibb.co/zVSbZm5v/6.jpg',
  'https://i.ibb.co/TxqRvrms/7.jpg',
  'https://i.ibb.co/RTWcjGrw/8.jpg'
]

const carouselElement = ref<HTMLElement | null>(null)

onMounted(() => {
  if (carouselElement.value) {
    // Created here (before window load) so Bootstrap's data-api picks up this
    // instance and its 3s interval instead of creating a default 5s one
    new bootstrap.Carousel(carouselElement.value, { interval: 3000, pause: false })
  }
})
</script>
