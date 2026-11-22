import type { PlayerEquipmentResponse } from "../types/PlayerEquipmentResponse";
import type { PlayerEquipmentSlot } from "../types/PlayerEquipmentSlot";

const professionSlotPattern = /^(.+?)(Charm|Instrument)$/;

export const equipmentSlotLabel = (slot: PlayerEquipmentSlot) =>
  slot.slot_type.replace(/([a-z])([A-Z])/g, "$1 $2");

export function groupProfessionEquipment(slots: readonly PlayerEquipmentSlot[]) {
  const bySkill = new Map<string, PlayerEquipmentSlot[]>();
  for (const slot of slots) {
    const skill = professionSlotPattern.exec(slot.slot_type)?.[1];
    if (!skill) continue;
    const group = bySkill.get(skill);
    if (group) group.push(slot);
    else bySkill.set(skill, [slot]);
  }
  return bySkill;
}

export function equipmentView(data: PlayerEquipmentResponse | null | undefined, selectedId: string | null) {
  const baseSlots = data?.slots ?? [];
  const presets = data?.presets ?? [];
  // The game stores index 0 in equipment_state. Only additional loadouts have preset rows.
  const base = {
    id: "base",
    index: 0,
    active: data?.presets != null && !presets.some((preset) => preset.active),
    slots: baseSlots,
  };
  const loadouts = [base, ...presets];
  const active = loadouts.find((loadout) => loadout.active);
  const selected = loadouts.find((loadout) => loadout.id === selectedId) ?? active ?? base;
  const slots = selected === base
    ? baseSlots
    : [
        // The head artifact (and the profession slots shown on Skills) is shared.
        ...baseSlots.filter((slot) => slot.slot_type === "HeadArtifact"),
        ...selected.slots.filter((slot) => slot.slot_type !== "HeadArtifact"),
      ];
  return {
    active,
    selected,
    loadouts,
    slots: slots.filter((slot) => !professionSlotPattern.test(slot.slot_type)),
  };
}
