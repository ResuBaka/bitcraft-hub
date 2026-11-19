import type { MaybeRefOrGetter, Ref } from "vue";
import type { PlayerCraft } from "~/types/PlayerCraft";
import { craftClaimOptions, craftMatchesClaims } from "~/utils/craftClaimFilter";

export function useCraftClaimFilter(
  selected: Ref<string[]>,
  crafts: MaybeRefOrGetter<PlayerCraft[]>,
) {
  const options = computed(() => craftClaimOptions(toValue(crafts), selected.value));
  const filtered = computed(() =>
    toValue(crafts).filter((craft) => craftMatchesClaims(craft, selected.value)),
  );
  return { options, filtered };
}
