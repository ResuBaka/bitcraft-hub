type ActiveFetchOptions = Pick<
  NonNullable<Parameters<typeof useFetchMsPack<unknown>>[1]>,
  "deep" | "query"
>;

/** Load a panel's data only in the browser while that panel is in use. */
export function useActiveFetchMsPack<T>(
  key: string,
  active: () => boolean,
  request: () => string,
  options: ActiveFetchOptions = {},
) {
  const result = useFetchMsPack<T>(request, {
    ...options,
    key,
    immediate: false,
    server: false,
    watch: false,
  });
  onMounted(() => {
    if (active()) void result.execute();
  });
  watch([active, request, () => toValue(options.query)], ([isActive]) => {
    if (isActive) void result.execute();
    else result.clear();
  });
  if (import.meta.client) onScopeDispose(result.clear);
  return result;
}
