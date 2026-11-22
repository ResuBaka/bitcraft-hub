import { expect, test } from "bun:test";
import type { PlayerEquipmentResponse } from "../app/types/PlayerEquipmentResponse";
import { equipmentView, groupProfessionEquipment } from "../app/utils/playerEquipment";
import { equipmentPlayerSection } from "../app/utils/playerSections";

const hand = { slot_type: "MainHand", item: null };
const clothing = { slot_type: "HeadClothing", item: null };
const artifact = { slot_type: "HeadArtifact", item: null };
const savedClothing = {
  slot_type: "HeadClothing",
  item: { item_id: 200, item_type: "Item" as const, name: "Leather Cap", icon_asset_name: null, tier: null, rarity: null, stats: null },
};
const charm = { slot_type: "ForestryCharm", item: null };
const instrument = { slot_type: "ForestryInstrument", item: null };
const mining = { slot_type: "MiningCharm", item: null };
const data: PlayerEquipmentResponse = {
  slots: [hand, artifact, clothing, charm, instrument, mining],
  presets: [
    { id: "10", index: 1, active: true, slots: [hand, savedClothing, mining] },
  ],
};

test("exposes Equipment as a player section", () => {
  expect(equipmentPlayerSection.value).toBe("equipment");
  expect(equipmentPlayerSection.slug).toBe("equipment");
});

test("groups charm and instrument slots on the corresponding skill, even when empty", () => {
  const groups = groupProfessionEquipment(data.slots);
  expect([...groups]).toEqual([["Forestry", [charm, instrument]], ["Mining", [mining]]]);
});

test("shows Slot 1 alongside the active preset and uses each loadout's own gear", () => {
  const current = equipmentView(data, null);
  expect(current.loadouts.map(({ index, active }) => [index, active])).toEqual([[0, false], [1, true]]);
  expect(current.active?.id).toBe("10");
  expect(current.selected.id).toBe("10");
  expect(current.slots).toEqual([artifact, hand, savedClothing]);

  const saved = equipmentView(data, "base");
  expect(saved.active?.id).toBe("10");
  expect(saved.selected.id).toBe("base");
  expect(saved.slots).toEqual([hand, artifact, clothing]);
  expect(equipmentView(data, "missing").selected.id).toBe("10");
});

test("marks Slot 1 active when no other preset is active, but not when preset data is unavailable", () => {
  expect(equipmentView({ ...data, presets: [] }, null).active?.id).toBe("base");
  const updated = {
    ...data,
    presets: data.presets!.map((preset) => ({ ...preset, active: false })),
  };
  expect(equipmentView(updated, null).selected.id).toBe("base");
  expect(equipmentView(updated, "10").slots).toEqual([artifact, hand, savedClothing]);
  const unknown = equipmentView({ ...data, presets: null }, null);
  expect(unknown.active).toBeUndefined();
  expect(unknown.selected.id).toBe("base");
  expect(unknown.slots).toEqual([hand, artifact, clothing]);
});
