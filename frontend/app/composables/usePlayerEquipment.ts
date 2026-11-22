import type { PlayerEquipmentResponse } from "~/types/PlayerEquipmentResponse";

export function usePlayerEquipment(playerId: () => string, active: () => boolean, skillsActive: () => boolean) {
  const request = () => `/api/bitcraft/players/${playerId()}/equipment`;
  const equipment = useFetchMsPack<PlayerEquipmentResponse>(request, {
    server: false,
    immediate: false,
    watch: false,
  });
  const toolPowers = useFetchMsPack<Record<number, number>>("/api/bitcraft/equipment-tool-powers", {
    server: false,
    immediate: false,
    watch: false,
  });
  onMounted(() => {
    if (active()) void equipment.execute();
    if (skillsActive()) void toolPowers.execute();
  });
  watch([active, request], ([enabled]) => {
    if (enabled) void equipment.execute();
    else equipment.clear();
  });
  watch(skillsActive, (enabled) => {
    if (enabled) void toolPowers.execute();
  });
  if (import.meta.client) {
    watch(active, (enabled, _, cleanup) => {
      if (!enabled) return;
      const timer = setInterval(() => {
        if (document.visibilityState === "visible") void equipment.refresh({ dedupe: "defer" });
      }, 30_000);
      cleanup(() => clearInterval(timer));
    }, { immediate: true });
  }
  return { equipment, toolPowers: toolPowers.data };
}
