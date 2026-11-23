import { expect, test } from "bun:test";
import {
  characterStatGroups,
  createDetailedStatFormatter,
  professionStatGroups,
} from "../app/utils/playerDetailedStats";

const format = createDetailedStatFormatter("en-US");
const displayed = (group: ReturnType<typeof characterStatGroups>[number]) =>
  Object.fromEntries(group.rows.map((stat) => [stat.label, format(stat)]));

test("formats character stats like the game panel and adds active and passive regen", () => {
  const values: (number | null)[] = Array(86).fill(null);
  Object.assign(values, { 2: 7.25, 3: 14, 4: 1, 7: 0, 15: 1.2, 16: 1.09, 47: 0, 48: -0.75 });
  const [base, regen, combat, allProfessions] = characterStatGroups(values);
  expect(displayed(base)["Movement Multiplier"]).toBe("100%");
  expect(displayed(regen)["Total Health Regen"]).toBe("+7.25 / second");
  expect(displayed(regen)["Total Stamina Regen"]).toBe("+13.25 / second");
  expect(displayed(combat).Armor).toBe("0");
  expect(displayed(combat).Accuracy).toBe("—");
  expect(displayed(allProfessions)["Crafting Speed"]).toBe("+20%");
  expect(displayed(allProfessions)["Gathering Speed"]).toBe("+9%");
  values[47] = null;
  expect(displayed(characterStatGroups(values)[1])["Total Health Regen"]).toBe("—");
});

test("shows profession power with the equipped tool and keeps crit bonus distinct from action multiplier", () => {
  const values: (number | null)[] = Array(86).fill(null);
  Object.assign(values, { 24: 1.13, 37: 4, 46: 5, 58: 0.05, 66: 0, 70: 0.72, 78: 0.54 });
  const metadata = { slots_by_skill: { 5: 0 }, power_by_item: { 42: 23 } };
  const toolbelt = { pockets: [{ contents: { item_id: 42, item_type: "Item" } }] };
  const groups = professionStatGroups(values, toolbelt, metadata);
  expect(groups.map((group) => group.title)).toEqual(["Forager", "Miner"]);
  expect(displayed(groups.find((group) => group.title === "Miner")!)).toEqual({
    "Total Power": "27",
    Speed: "+13%",
    "Crit Chance": "5%",
    "Crit Multiplier": "0.72",
  });
  expect(displayed(groups.find((group) => group.title === "Forager")!)["Crit Chance"]).toBe("0%");
  expect(displayed(professionStatGroups(values, undefined, metadata)[1])["Total Power"]).toBe("—");
  const emptySlot = professionStatGroups(values, { pockets: [{ contents: null }] }, metadata);
  expect(displayed(emptySlot.find((group) => group.title === "Miner")!)["Total Power"]).toBe("5");
});
