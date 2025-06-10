let app = createApp({
    setup() {
        return {
            loggedin, onLogout, isAdmin
        }
    }
})
app.config.compilerOptions.delimiters = ['${', '}']
app.mount('#app')

const carousel = new bootstrap.Carousel("#bannerCarousel", {
    interval: 3000,
    pause: false
})

