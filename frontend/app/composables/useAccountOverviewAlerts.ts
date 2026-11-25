import type { InjectionKey } from "vue";
import { useEventListener, useLocalStorage } from "@vueuse/core";
import { accountOverviewBlocks } from "~/utils/accountOverviewBlocks";
import {
  alertsAllowed,
  overviewAlertTypes,
  type AlertType,
  type AlertPreferences,
  type AlertObservation,
} from "~/utils/accountOverviewAlerts";
import {
  emptyJournal,
  flushPassiveGroups,
  pruneJournal,
  recordObservation,
  type AlertEntry,
  type AlertJournal,
} from "~/utils/accountOverviewAlertJournal";
import type { TrackedPlayer } from "~/utils/accountOverviewShare";
import { alertSoundLibraryKey, useAlertSoundLibrary } from "~/composables/useAlertSoundLibrary";
import { createAlertSoundPlayback } from "~/utils/alertSoundPlayback";

type Notify = (id: string, observation: AlertObservation) => void;
const journalKey = "bitcraft-hub@overview-alert-journal";
export const overviewAlertHistoryKey: InjectionKey<{
  entries: Ref<AlertEntry[]>;
  clear: () => void;
}> = Symbol("overview-alert-history");
const notifyKey: InjectionKey<Notify> = Symbol("account-overview-alert");
export const testOverviewSoundKey: InjectionKey<(player?: string, type?: AlertType) => void> =
  Symbol("test-overview-sound");
export const overviewAlertPreferencesKey: InjectionKey<Ref<AlertPreferences>> = Symbol(
  "account-overview-alert-preferences",
);

export function provideAccountOverviewAlerts(players: Ref<TrackedPlayer[]>) {
  const preferences = useLocalStorage<AlertPreferences>(
    "bitcraft-hub@overview-alerts",
    {
      enabled: false,
      volume: 100,
      enabledTypes: Object.keys(overviewAlertTypes) as AlertType[],
      mutedPlayers: [],
      mutedBlocks: {},
    },
    { initOnMounted: true, mergeDefaults: true },
  );
  provide(overviewAlertPreferencesKey, preferences);
  const library = useAlertSoundLibrary();
  provide(alertSoundLibraryKey, library);
  const toast = useToast();
  const soundNotice = "overview-alert-sound-notice";
  const playback = createAlertSoundPlayback({
    volume: () => preferences.value.volume,
    allowed: (id, type) => alertsAllowed(preferences.value, id, type),
    sound: library.sound,
    notice(message, retry) {
      toast.add({
        id: soundNotice,
        title: "Alert sound",
        description: message,
        duration: retry ? 0 : 6000,
        actions: retry ? [{ label: "Play sound", closeOnClick: false, onClick: retry }] : [],
      });
    },
    started() {
      if (toast.toasts.value.some((t) => t.id === soundNotice && t.open)) toast.remove(soundNotice);
    },
  });
  provide(testOverviewSoundKey, playback.test);
  let journal = emptyJournal();
  const entries = shallowRef<AlertEntry[]>([]);
  const unseenCount = ref(0);
  const tabIsActive = () => document.visibilityState === "visible" && document.hasFocus();
  function clearWhenActive() {
    if (tabIsActive()) unseenCount.value = 0;
  }
  useEventListener("focus", clearWhenActive);
  useEventListener(
    () => (import.meta.client ? document : null),
    "visibilitychange",
    clearWhenActive,
  );
  let disposed = false;
  let storageAvailable = true;
  let passiveTimer: ReturnType<typeof setTimeout> | undefined;
  function passiveAllowed(id: string) {
    return players.value.some((p) => p.id === id)
      ? alertsAllowed(preferences.value, id, "passive-ready")
      : null;
  }
  function schedulePassiveFlush() {
    clearTimeout(passiveTimer);
    let next = Infinity;
    for (const group of journal.passiveGroups ?? []) {
      const allowed = passiveAllowed(group.player.id);
      if (allowed !== null) next = Math.min(next, allowed ? group.until : 0);
    }
    if (Number.isFinite(next))
      passiveTimer = setTimeout(() => updateJournal(), Math.max(0, next - Date.now()));
  }
  function readJournal() {
    if (!storageAvailable) return;
    try {
      const saved = JSON.parse(localStorage.getItem(journalKey) ?? "null");
      journal = saved?.marks && Array.isArray(saved.entries) ? saved : emptyJournal();
    } catch {
      storageAvailable = false;
    }
  }
  function updateJournal(change?: (journal: AlertJournal) => AlertEntry | void) {
    const commit = () => {
      if (disposed) return;
      readJournal();
      const now = Date.now();
      pruneJournal(journal, now);
      const batches = flushPassiveGroups(journal, now, passiveAllowed);
      const entry = change?.(journal);
      entries.value = [...journal.entries];
      try {
        if (storageAvailable) localStorage.setItem(journalKey, JSON.stringify(journal));
      } catch {
        storageAvailable = false;
      }
      schedulePassiveFlush();
      for (const batch of batches) deliver(batch);
      if (entry) deliver(entry);
    };
    // Two open dashboards may receive the same websocket update together.
    if (navigator.locks) void navigator.locks.request(journalKey, commit);
    else commit();
  }
  function reloadHistory(e: StorageEvent) {
    if (e.key !== journalKey && e.key !== null) return;
    readJournal();
    entries.value = journal.entries;
    schedulePassiveFlush();
  }
  let cleanup: ReturnType<typeof setInterval>;
  onMounted(() => {
    updateJournal();
    window.addEventListener("storage", reloadHistory);
    cleanup = setInterval(() => updateJournal(), 60 * 60 * 1000);
  });
  provide(overviewAlertHistoryKey, {
    entries,
    clear: () =>
      updateJournal((j) => {
        j.entries = [];
      }),
  });
  provide(notifyKey, (id, observation) => {
    const player = players.value.find((p) => p.id === id);
    if (!player) return;
    updateJournal((j) =>
      recordObservation(
        j,
        player,
        observation,
        alertsAllowed(preferences.value, id, observation.type),
        Date.now(),
      ),
    );
  });
  function deliver(entry: AlertEntry) {
    if (!tabIsActive()) unseenCount.value++;
    const { player, type, message: description } = entry;
    const id = player.id;
    const block = overviewAlertTypes[type].block;
    const source = accountOverviewBlocks.find((b) => b.id === block)!;
    toast.add({
      id: entry.id,
      title: `${player.name} · ${source.name}`,
      icon: source.icon,
      description,
    });
    playback.enqueue(id, type);
  }
  watch(
    preferences,
    () => {
      playback.update();
      if (journal.passiveGroups?.length) updateJournal();
    },
    { deep: true },
  );
  watch(players, schedulePassiveFlush);
  onBeforeUnmount(() => {
    disposed = true;
    clearTimeout(passiveTimer);
    clearInterval(cleanup);
    window.removeEventListener("storage", reloadHistory);
    playback.dispose();
    toast.remove(soundNotice);
  });
  return { preferences, unseenCount };
}

export function useOverviewNotify() {
  return inject(notifyKey, null);
}
