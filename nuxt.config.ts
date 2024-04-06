// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  devtools: { enabled: true },
  ssr: false,
  modules: [
    'vuetify-nuxt-module'
  ],
  vuetify: {
    moduleOptions: {
      ssrClientHints: {
        prefersColorSchemeOptions: {
          useBrowserThemeOnly: true,
        }
      }
    },
  },
  vite: {
    $server: {
      build: {
        target: 'esnext',
      }
    }
  }
})
