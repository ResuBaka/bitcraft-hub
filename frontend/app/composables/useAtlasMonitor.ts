import { useIntervalFn, useLocalStorage } from "@vueuse/core";
import { normalizeAtlasResourceColor } from "~/utils/atlasResources";
import {
  ATLAS_MONITOR_KEY,
  DEFAULT_TRACKED_PLAYER_COLOR,
  MAX_FAVORITES,
  MAX_TRACKED_TARGETS,
  monitorKey,
  monitorEntityId,
  readAtlasMonitor,
  trackedAtlasPlayers,
  type MonitorPreferences,
  type MonitorRosters,
  type MonitorTarget,
} from "~/utils/atlasMonitor";

export function useAtlasMonitor(base: string | undefined) {
  const preferences = useLocalStorage<MonitorPreferences>(
    ATLAS_MONITOR_KEY,
    { tracking: [], favorites: [], playerColors: {} },
    { serializer: { read: readAtlasMonitor, write: JSON.stringify }, writeDefaults: false },
  );
  const rosters = shallowRef<MonitorRosters>({});
  const rosterError = ref(false);
  const rosterLoading = ref(false);
  const claimIds = computed(() =>
    preferences.value.tracking
      .filter(({ kind }) => kind === "claim")
      .map(({ id }) => id)
      .sort()
      .join(","),
  );
  const targets = computed(() => trackedAtlasPlayers(preferences.value.tracking, rosters.value));
  let request: AbortController | undefined;

  async function refreshRosters() {
    if (!base || !claimIds.value || request || (typeof document !== "undefined" && document.hidden))
      return;
    const controller = new AbortController();
    request = controller;
    rosterLoading.value = true;
    try {
      const data = await $fetch<MonitorRosters>(`${base}claim-rosters`, {
        query: { ids: claimIds.value },
        signal: controller.signal,
        timeout: 5_000,
        retry: 0,
      });
      if (!controller.signal.aborted) {
        rosters.value = data;
        rosterError.value = false;
      }
    } catch {
      if (!controller.signal.aborted) rosterError.value = true;
    } finally {
      if (request === controller) {
        request = undefined;
        rosterLoading.value = false;
      }
    }
  }

  const { pause, resume } = useIntervalFn(refreshRosters, 30_000, { immediate: false });
  watch(
    claimIds,
    (ids) => {
      request?.abort();
      request = undefined;
      rosterLoading.value = rosterError.value = false;
      const wanted = new Set(ids.split(","));
      rosters.value = Object.fromEntries(
        Object.entries(rosters.value).filter(([id]) => wanted.has(id)),
      );
      if (!ids) pause();
      else {
        resume();
        void refreshRosters();
      }
    },
    { immediate: true },
  );
  onScopeDispose(() => request?.abort());

  function toggle(list: "tracking" | "favorites", target: MonitorTarget) {
    if (list === "tracking" && target.kind === "resource") return;
    const rows = preferences.value[list];
    const key = monitorKey(target);
    if (rows.some((row) => monitorKey(row) === key)) {
      preferences.value[list] = rows.filter((row) => monitorKey(row) !== key);
    } else if (rows.length < (list === "tracking" ? MAX_TRACKED_TARGETS : MAX_FAVORITES)) {
      preferences.value[list] = [...rows, target];
    }
  }

  function setPlayerColor(id: string, value: string | undefined) {
    if (!monitorEntityId(id)) return;
    const color = normalizeAtlasResourceColor(value);
    if (value !== undefined && !color) return;
    const next = { ...preferences.value.playerColors };
    if (color && color !== DEFAULT_TRACKED_PLAYER_COLOR) next[id] = color;
    else delete next[id];
    preferences.value.playerColors = next;
  }

  return {
    preferences,
    rosters,
    rosterLoading,
    rosterError,
    refreshRosters,
    targets,
    toggle,
    setPlayerColor,
  };
}
