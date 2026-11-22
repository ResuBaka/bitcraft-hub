import { refDebounced } from "@vueuse/core";
import type { LocationQueryRaw } from "vue-router";
import { catalogQueryPage, catalogQueryText } from "~/utils/catalogQuery";

export function useCatalogControls() {
  const route = useRoute();
  const router = useRouter();
  const searchTerm = computed(() => catalogQueryText(route.query.search));
  const search = ref(searchTerm.value ?? "");
  const debouncedSearch = refDebounced(search, 200, { maxWait: 800 });

  function change(patch: LocationQueryRaw, replace = false) {
    const query = { ...route.query, ...patch };
    const target = { query, hash: route.hash };
    if (replace) void router.replace(target);
    else void router.push(target);
  }

  watch(searchTerm, (value) => {
    if (catalogQueryText(search.value) !== value) search.value = value ?? "";
  });
  watch(debouncedSearch, (value) => {
    const next = catalogQueryText(value);
    if (next !== searchTerm.value) change({ search: next, page: undefined }, true);
  });

  const page = computed({
    get: () => catalogQueryPage(route.query.page),
    set: (value) => {
      if (value !== page.value) change({ page: value > 1 ? String(value) : undefined });
    },
  });

  function filter<T>(
    key: string,
    decode: (value: string | undefined) => T,
    encode: (value: T) => string | undefined,
  ) {
    return computed({
      get: () => decode(catalogQueryText(route.query[key])),
      set: (value) => {
        const encoded = encode(value);
        if (encoded !== catalogQueryText(route.query[key])) {
          change({ [key]: encoded, page: undefined });
        }
      },
    });
  }

  return { search, searchTerm, page, filter };
}
