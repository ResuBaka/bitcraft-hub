// https://nuxt.com/docs/api/configuration/nuxt-config
const scheduledTasks  = process.env.SCHEDULE_TASKS_ENABLED === "true" ? {
  '*/1 * * * *': ['state:refresh'],
} : undefined;


export default defineNuxtConfig({
  devtools: { enabled: true },
  ssr: false,
  nitro: {
    experimental: {
      websocket: true,
      tasks: true,
    },
    scheduledTasks,
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
