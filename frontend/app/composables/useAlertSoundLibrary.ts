import type { InjectionKey } from "vue";
import { useLocalStorage } from "@vueuse/core";
import {
  readSoundLibrary,
  removeAlertSound,
  renameAlertSound,
  resolveAlertSound,
  saveAlertSound,
  selectedClip,
  soundPrefix,
  soundScope,
  type AlertSound,
  type SoundChoices,
} from "~/utils/accountOverviewSound";
import type { AlertType } from "~/utils/accountOverviewAlerts";

export const alertSoundLibraryKey: InjectionKey<ReturnType<typeof useAlertSoundLibrary>> =
  Symbol("alert-sound-library");

export function useAlertSoundLibrary() {
  const clips = shallowRef<AlertSound[]>([]);
  const choices = useLocalStorage<SoundChoices>(
    "bitcraft-hub@overview-sound-choices",
    {},
    { initOnMounted: true },
  );
  const reload = () => {
    clips.value = readSoundLibrary();
  };
  function onStorage(event: StorageEvent) {
    if (event.key === null || event.key.startsWith(soundPrefix)) reload();
  }
  onMounted(() => {
    reload();
    window.addEventListener("storage", onStorage);
  });
  onBeforeUnmount(() => window.removeEventListener("storage", onStorage));
  return {
    clips,
    selected: (player?: string, type?: AlertType) =>
      selectedClip(clips.value, choices.value, player, type)?.id ?? "",
    select(id: string, player?: string, type?: AlertType) {
      choices.value[soundScope(player, type)] = id;
    },
    sound: (player?: string, type?: AlertType) =>
      resolveAlertSound(clips.value, choices.value, player, type)?.url,
    async upload(file: File) {
      await saveAlertSound(file);
      reload();
    },
    rename(id: string, name: string) {
      renameAlertSound(id, name);
      reload();
    },
    remove(id: string) {
      removeAlertSound(id);
      for (const [scope, assigned] of Object.entries(choices.value))
        if (assigned === id) choices.value[scope] = "";
      reload();
    },
  };
}
