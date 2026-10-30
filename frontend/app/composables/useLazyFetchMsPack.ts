import { unpack } from "msgpackr/unpack";

export function useLazyFetchMsPack<DataT, ErrorT = undefined>(
  ...args: Parameters<typeof useLazyFetch<DataT, ErrorT>>
): ReturnType<typeof useLazyFetch<DataT, ErrorT>> {
  const config = useRuntimeConfig();
  const [request, options] = args;
  return useLazyFetch<DataT, ErrorT>(request, {
    // Keep Nuxt's payload key identical during SSR and client hydration.
    baseURL: config.public.api.base,
    timeout: 5000,
    ...options,
    async onRequest(context) {
      if (import.meta.server && config.apiBase && !options?.baseURL) {
        context.options.baseURL = config.apiBase;
      }
      const hooks = options?.onRequest;
      for (const hook of Array.isArray(hooks) ? hooks : hooks ? [hooks] : []) {
        await hook(context);
      }
    },
    headers: {
      Accept: "application/vnd.msgpack",
    },
    transform: async (response: Blob) => {
      try {
        return unpack((await response.arrayBuffer()) as Buffer, {
          int64AsType: "auto",
        });
      } catch (e) {
        console.error("msgpack Parsing Error:", e);
        throw new Error("Failed to parse msgpack response.");
      }
    },
  });
}
