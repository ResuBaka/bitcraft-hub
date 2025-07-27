// https://nuxt.com/docs/api/configuration/nuxt-config
import colors from "vuetify/util/colors";

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
  build: {
    transpile: ['vue-sonner']
  },
  modules: [
    // '@artmizu/nuxt-prometheus',
    'vuetify-nuxt-module',
    '@pinia/nuxt',
  ],
  css: ['@/assets/css/custom.scss'],
  vuetify: {
    moduleOptions: {
      ssrClientHints: {
        prefersColorSchemeOptions: {
          useBrowserThemeOnly: true,
        }
      },
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
      },
      theme: {
        themes: {
          light: {
            variables: {
              'color-tier-1': colors.grey.darken4,
              'color-tier-2': colors.orange.darken4,
              'color-tier-3': colors.green.darken4,
              'color-tier-4': colors.blue.darken4,
              'color-tier-5': colors.purple.darken4,
              'color-tier-6': colors.red.darken4,
              'color-tier-7': colors.yellow.darken4,
              'color-tier-8': colors.teal.darken4,
              'color-tier-9': colors.deepPurple.darken4,
              'color-tier-10': colors.deepPurple.darken4,
            }
          },
          dark: {
            variables: {
              'color-tier-1': colors.grey.lighten1,
              'color-tier-2': colors.orange.base,
              'color-tier-3': colors.green.base,
              'color-tier-4': colors.blue.base,
              'color-tier-5': colors.purple.base,
              'color-tier-6': colors.red.base,
              'color-tier-7': colors.yellow.base,
              'color-tier-8': colors.teal.base,
              'color-tier-9': colors.deepPurple.base,
              'color-tier-10': colors.deepPurple.base,
            }
          }
        }
      }
    }
  },

  vite: {
    experimental: {
      enableNativePlugin: true,
    },
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
      disable: {
        refresh: false,
      }
    },
    public: {
      iconDomain: "",
      api: {
        base: "",
        websocket: "",
      },
    },
  },
  compatibilityDate: '2024-08-10'
})