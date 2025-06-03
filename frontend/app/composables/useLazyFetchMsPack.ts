import { unpack } from "msgpackr/unpack";
import type { UseFetchOptions } from "nuxt/app";

export function useLazyFetchMsPack<T>(
  request: string | (() => string),
  options: UseFetchOptions<T> = {},
) {
  return useLazyFetch(request, {
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
