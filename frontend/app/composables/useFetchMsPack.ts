import { unpack } from "msgpackr/unpack";

export function useFetchMsPack<DataT, ErrorT = undefined>(
  ...args: Parameters<typeof useFetch<DataT, ErrorT>>
): ReturnType<typeof useFetch<DataT, ErrorT>> {
  const {
    public: { api },
  } = useRuntimeConfig();
  const [request, options] = args;

  // @ts-ignore
  return useFetch<DataT, ErrorT>(request, {
    baseURL: api.base,
    ...options,
    headers: {
      Accept: "application/vnd.msgpack",
    },
    transform: async (response: Blob) => {
      try {
        return unpack((await response.arrayBuffer()) as Buffer, {
          // @ts-ignore
          int64AsType: "auto",
        });
      } catch (e) {
        console.error("msgpack Parsing Error:", e);
        throw new Error("Failed to parse msgpack response.");
      }
    },
  });
}
