import { useLocalStorage } from "@vueuse/core";
import type { PlayerCraft } from "~/types/PlayerCraft";

export function useCraftAcknowledgements(
  playerId: string,
  kind: PlayerCraft["kind"],
  data: Ref<PlayerCraft[] | null | undefined>,
) {
  const seen = useLocalStorage(`bitcraft-hub@craft-seen:${kind}:${playerId}`, new Set<string>());
  // Prune against the player's full snapshot, never the selected claim filter.
  watch(
    data,
    (rows) => {
      if (!rows || !seen.value.size) return;
      const liveIds = new Set(rows.filter((c) => c.kind === kind).map((c) => c.id));
      for (const id of seen.value) {
        if (!liveIds.has(id)) seen.value.delete(id);
      }
    },
    { immediate: true },
  );
  function acknowledge(ids: string[]) {
    for (const id of ids) seen.value.add(id);
  }
  return { seen, acknowledge };
}
