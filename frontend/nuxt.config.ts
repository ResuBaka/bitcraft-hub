// https://nuxt.com/docs/api/configuration/nuxt-config

export default defineNuxtConfig({
  devtools: { enabled: true },
  nitro: {
    preset: "bun"
  },
  modules: [
    '@nuxt/ui',
    '@pinia/nuxt',
  ],
  css: ['@/assets/css/main.css'],
  vite: {
    optimizeDeps: {
      include: [
        '@vue/devtools-core',
        '@vue/devtools-kit',
        '@antfu/utils',
        '@vueuse/shared',
        'msgpackr/unpack',
        '@vueuse/core',
      ]
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