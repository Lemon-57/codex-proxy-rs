import { createApp } from 'vue'
import App from './App.vue'
import { i18n } from './i18n'

import { authPlugin } from './plugins/auth'
import { router } from './router'
import { pinia } from './stores'
import { useThemeStore } from './stores/modules/theme'
import '@fontsource-variable/inter'
import '@fontsource-variable/jetbrains-mono'
import '@codex-proxy/ui/styles.css'

import './styles/index.css'

const app = createApp(App)

app.use(pinia)
app.use(i18n)

useThemeStore(pinia).initializeTheme()

app.use(router)
app.use(authPlugin)
app.mount('#app')
