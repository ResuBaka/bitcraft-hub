import type { ItemSlotResolved } from "~/types/ItemSlotResolved";
import type { ResolvedInventory } from "~/types/ResolvedInventory";

export const pocketMatchesSearch = (pocket: ItemSlotResolved, search: string) => {
  return (
    !!pocket.contents &&
    pocket.contents.item.name.toLowerCase().includes(search.trim().toLowerCase())
  );
};

export const inventoryMatchesSearch = (inventory: ResolvedInventory, search: string) => {
  return inventory.pockets.some((pocket) => pocketMatchesSearch(pocket, search));
};
