import { deepMerge } from "@antfu/utils";
import { skipHydrate } from "pinia";
import { onMounted } from "vue";

const IN_BROWSER = typeof window !== "undefined";

export type ConfigState = {
  version: 1;
  theme: string;
  websocket: {
    enabled_default: boolean;
  };
  show_region_as_number: boolean;
};

export const DEFAULT_CONFIG: ConfigState = {
  version: 1,
  theme: "system",
  websocket: {
    enabled_default: false,
  },
  show_region_as_number: true,
};

export const useConfigStore = defineStore("config", () => {
  const state = reactive(deepMerge({}, DEFAULT_CONFIG));
  let watchInitialized = false;

  function initWatch() {
    if (watchInitialized) return;

    watchInitialized = true;
    watch(state, () => {
      save();
    });
  }

  function load() {
    if (import.meta.server) {
      return;
    }

    if (import.meta.client) {
      const stored = localStorage.getItem("b-tool@config");
      const data = stored ? JSON.parse(stored) : {};
      const needsRefresh = data.version === state.version;

      data.version = state.version;
      const newState = deepMerge(state, data);
      Object.assign(state, newState);
      initWatch();
      if (needsRefresh) {
        save();
      }
    }
  }

  function save() {
    if (!IN_BROWSER) return;

    console.log("SAVE", JSON.stringify(state));
    localStorage.setItem("b-tool@config", JSON.stringify(state, null, 2));
  }

  function reset() {
    if (!IN_BROWSER) return;

    Object.assign(state, deepMerge({}, DEFAULT_CONFIG));

    save();
  }

  onMounted(() => {
    load();
  });

  return {
    ...toRefs(state),
    load,
    save,
    reset,
  };
});
