// https://nuxt.com/docs/api/configuration/nuxt-config

export default defineNuxtConfig({
  devtools: { enabled: process.env.NUXT_DEVTOOLS_ENABLED === "true" },
  experimental: {
    defaults: {
      // Large tables and catalogs shouldn't queue an idle task for every link.
      nuxtLink: { prefetchOn: { visibility: false, interaction: true } },
    },
  },
  devServer: { host: 'localhost' },
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
    apiBase: "",
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
      siteUrl: "",
      socialImage: "/Icons/SkillIconANY.png",
      api: {
        base: "",
        websocket: "",
      },
    },
  },
  compatibilityDate: '2024-08-10'
})
