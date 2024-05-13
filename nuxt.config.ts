// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  devtools: { enabled: true },
  ssr: false,
  nitro: {
    experimental: {
      websocket: true,
      tasks: true,
    },
    preset: "bun"
  },
  modules: [
    'vuetify-nuxt-module',
    '@pinia/nuxt',
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
  },
  runtimeConfig: {
    bitcraft: {
      websocket: {
        enabled: false
      }
    }
  }
})
