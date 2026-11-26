export type AtlasFetch<Query, Data> = (
  query: Query,
  signal: AbortSignal,
  onCached?: (data: Data) => void,
) => Promise<Data>;

type Store = {
  read: (key: string) => Promise<unknown>;
  write: (key: string, data: unknown) => Promise<void>;
};

const CACHE_NAME = "bitcraft-atlas-overlays-v1";
const MAX_ENTRIES = 48;
const MAX_ENTRY_LENGTH = 384_000;
const MAX_AGE_MS = 15 * 60_000;
let queuedWrite = Promise.resolve();

const browserStore: Store = {
  async read(key) {
    const cache = await globalThis.caches?.open(CACHE_NAME);
    const entry = await cache?.match(key);
    if (entry && Date.now() - Number(entry.headers.get("x-atlas-saved-at")) < MAX_AGE_MS)
      return entry.json();
  },
  async write(key, data) {
    const cache = await globalThis.caches?.open(CACHE_NAME);
    if (!cache) return;
    const body = JSON.stringify(data);
    if (body.length > MAX_ENTRY_LENGTH) return;
    await cache.put(
      key,
      new Response(body, {
        headers: {
          "content-type": "application/json",
          "x-atlas-saved-at": String(Date.now()),
        },
      }),
    );
    const keys = await cache.keys();
    await Promise.all(
      keys
        .filter((entry) => entry.url !== key)
        .slice(0, Math.max(0, keys.length - MAX_ENTRIES))
        .map((entry) => cache.delete(entry)),
    );
  },
};

export function atlasCacheKey(endpoint: string, query: object, base: string) {
  const url = new URL(endpoint, base);
  for (const [key, value] of Object.entries(query))
    if (value !== undefined) url.searchParams.set(key, String(value));
  url.searchParams.sort();
  return url.href;
}

export function withAtlasBrowserCache<Query extends object, Data>(
  endpoint: string,
  base: string,
  fetchFresh: (query: Query, signal: AbortSignal) => Promise<Data>,
  store: Store = browserStore,
): AtlasFetch<Query, Data> {
  return async (query, signal, onCached) => {
    const key = atlasCacheKey(endpoint, query, base);
    let pending = true;
    // A slow cache read must never delay the network or overwrite fresher data.
    if (onCached)
      void store
        .read(key)
        .then((data) => {
          if (pending && !signal.aborted && data !== undefined) onCached(data as Data);
        })
        .catch(() => {});
    try {
      const fresh = await fetchFresh(query, signal);
      if (!signal.aborted) {
        queuedWrite = queuedWrite.then(() => store.write(key, fresh)).catch(() => {});
      }
      return fresh;
    } finally {
      pending = false;
    }
  };
}
