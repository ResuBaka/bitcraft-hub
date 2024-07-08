// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  devtools: { enabled: true },
  ssr: false,
  nitro: {
    experimental: {
      websocket: true,
      tasks: true,
    },
    scheduledTasks: {
      // Run `cms:update` task every minute
      //'*/1 * * * *': ['state:refresh'],
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
    vuetifyOptions: {
      icons: {
        defaultSet: 'mdi',
        sets: [
          {
            name: 'mdi',
            cdn: 'https://cdn.jsdelivr.net/npm/@mdi/font@7.x/css/materialdesignicons.min.css'
          }
        ]
      }
    }
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
        enabled: false,
        url: ""
      },
      url: "",
      auth: {
        password: "",
      },
    },
    public: {
      iconDomain: "",
    }
  }
})
