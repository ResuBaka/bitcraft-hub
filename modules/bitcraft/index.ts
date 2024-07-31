// `nuxt/kit` is a helper subpath import you can use when defining local modules
// that means you do not need to add `@nuxt/kit` to your project's dependencies
import { createResolver, defineNuxtModule, addServerScanDir } from "nuxt/kit";
import { defu } from "defu";

export default defineNuxtModule({
  meta: {
    name: "bitcraft",
    configKey: "bitcraft",
  },
  defaults: {
    websocket: {
      enabled: false,
      url: "",
    },
    url: "",
    auth: {
      username: "token",
      password: "",
    },
    disable: {
      refresh: false,
    },
  },
  setup(options, nuxt) {
    nuxt.options.runtimeConfig.bitcraft = defu(
      nuxt.options.runtimeConfig.bitcraft,
      {
        websocket: {
          enabled: options.websocket.enabled,
        },
        url: options.url,
        auth: {
          username: "token",
          password: "",
        },
        disable: {
          refresh: false,
        },
      },
    );

    // Add an API route
    const resolver = createResolver(import.meta.url);

    addServerScanDir(resolver.resolve("./runtime/server"), { prepend: true });
  },
});
