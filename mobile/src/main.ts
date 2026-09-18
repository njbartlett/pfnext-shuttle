import { createApp } from 'vue'
import { IonicVue } from '@ionic/vue'
import { configureApi } from '@pfnext/shared'
import App from './App.vue'
import router from './router'

/* Ionic core and basic app styling */
import '@ionic/vue/css/core.css'
import '@ionic/vue/css/normalize.css'
import '@ionic/vue/css/structure.css'
import '@ionic/vue/css/typography.css'
import '@ionic/vue/css/padding.css'
import '@ionic/vue/css/text-alignment.css'

/* Follow the device light/dark setting */
import '@ionic/vue/css/palettes/dark.system.css'

import './theme.css'

// Mobile clients use the frozen /api/v1 contract with a bearer token
configureApi({
  baseUrl: import.meta.env.VITE_API_BASE_URL ?? 'https://anotherlevelfitness.uk/api/v1'
})

const app = createApp(App).use(IonicVue).use(router)

router.isReady().then(() => {
  app.mount('#app')
})
