import { unpack } from "msgpackr/unpack";

export function useLazyFetchMsPack<DataT, ErrorT = undefined>(
  ...args: Parameters<typeof useLazyFetch<DataT, ErrorT>>
): ReturnType<typeof useLazyFetch<DataT, ErrorT>> {
  const {
    public: { api },
  } = useRuntimeConfig();
  const [request, options] = args;
  return useLazyFetch<DataT, ErrorT>(request, {
    baseURL: api.base,
    ...options,
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
