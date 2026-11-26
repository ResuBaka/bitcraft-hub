// https://nuxt.com/docs/api/configuration/nuxt-config

export default defineNuxtConfig({
  app: { head: { link: [{ key: "favicon", rel: "icon", href: "/favicon.ico" }] } },
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
        'ol/Feature.js',
        'ol/Map.js',
        'ol/Overlay.js',
        'ol/View.js',
        'ol/control/defaults.js',
        'ol/extent.js',
        'ol/geom/Polygon.js',
        'ol/geom/Point.js',
        'ol/geom/MultiLineString.js',
        'ol/geom/MultiPolygon.js',
        'ol/geom/MultiPoint.js',
        'ol/interaction/defaults.js',
        'ol/layer/Tile.js',
        'ol/layer/Vector.js',
        'ol/proj/Projection.js',
        'ol/source/Vector.js',
        'ol/source/XYZ.js',
        'ol/style.js',
        'ol/tilegrid/TileGrid.js',
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
