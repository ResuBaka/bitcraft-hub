import type { ToolPowers } from "../types/CharacterStatsSnapshot";
import { compareSkillNames } from "./skillOrder";

type Unit = "points" | "percent" | "bonus" | "regen" | "decimal";
type Row = { label: string; value: number | null; unit: Unit; description?: string };
type Group = { title: string; rows: Row[] };
type Toolbelt = { pockets: { contents: { item_id: number; item_type: string } | null }[] };

// CharacterStatType discriminants from BitCraft's static_data.rs.
const Stat = {
  MaxHealth: 0,
  MaxStamina: 1,
  PassiveHealthRegen: 2,
  PassiveStaminaRegen: 3,
  Movement: 4,
  Armor: 7,
  Cooldown: 8,
  HuntingStrength: 9,
  Strength: 10,
  Cold: 11,
  Heat: 12,
  Evasion: 13,
  CraftingSpeed: 15,
  GatheringSpeed: 16,
  BuildingSpeed: 17,
  MaxSatiation: 19,
  ActiveHealthRegen: 47,
  ActiveStaminaRegen: 48,
  Accuracy: 51,
  ConstructionPower: 54,
  CartSpeed: 83,
  MountSpeed: 84,
  BoatSpeed: 85,
} as const;

const sum = (a: number | null, b: number | null) => (a === null || b === null ? null : a + b);
const row = (label: string, value: number | null, unit: Unit, description?: string): Row => ({
  label,
  value,
  unit,
  description,
});

export function characterStatGroups(values: (number | null)[]): Group[] {
  const read = (id: number) => values[id] ?? null;
  const stat = (label: string, id: number, unit: Unit = "points") => row(label, read(id), unit);
  return [
    {
      title: "Base",
      rows: [
        stat("Maximum Health", Stat.MaxHealth),
        stat("Maximum Stamina", Stat.MaxStamina),
        stat("Maximum Satiation", Stat.MaxSatiation),
        stat("Movement Multiplier", Stat.Movement, "percent"),
        stat("Cart Speed", Stat.CartSpeed, "percent"),
        stat("Mount Speed", Stat.MountSpeed, "percent"),
        stat("Boat Speed", Stat.BoatSpeed, "percent"),
      ],
    },
    {
      title: "Regen",
      rows: [
        stat("Passive Health Regeneration Rate", Stat.PassiveHealthRegen, "regen"),
        stat("Active Health Regeneration Rate", Stat.ActiveHealthRegen, "regen"),
        row(
          "Total Health Regen",
          sum(read(Stat.PassiveHealthRegen), read(Stat.ActiveHealthRegen)),
          "regen",
          "Active plus passive regeneration. Passive health regeneration requires time without damage and being out of combat.",
        ),
        stat("Passive Stamina Regeneration Rate", Stat.PassiveStaminaRegen, "regen"),
        stat("Active Stamina Regeneration Rate", Stat.ActiveStaminaRegen, "regen"),
        row(
          "Total Stamina Regen",
          sum(read(Stat.PassiveStaminaRegen), read(Stat.ActiveStaminaRegen)),
          "regen",
          "Active plus passive regeneration. Passive stamina regeneration requires time without spending stamina.",
        ),
      ],
    },
    {
      title: "Combat",
      rows: [
        stat("Armor", Stat.Armor),
        stat("Accuracy", Stat.Accuracy),
        stat("Evasion", Stat.Evasion),
        stat("Strength", Stat.Strength),
        stat("Hunting Strength", Stat.HuntingStrength),
        stat("Cooldown Multiplier", Stat.Cooldown, "percent"),
      ],
    },
    {
      title: "All Professions",
      rows: [
        stat("Gathering Speed", Stat.GatheringSpeed, "bonus"),
        stat("Crafting Speed", Stat.CraftingSpeed, "bonus"),
        stat("Building Speed", Stat.BuildingSpeed, "bonus"),
      ],
    },
    {
      title: "Environment",
      rows: [stat("Heat Protection", Stat.Heat), stat("Cold Protection", Stat.Cold)],
    },
  ];
}

// These four stat ranges share the same Forestry → Foraging order, except
// Cooking has no crit stats. Hexite Gathering was appended separately.
const professions: {
  title: string;
  skillName: string;
  skill: number;
  power: number;
  speed: number | null;
  chance: number | null;
  crit: number | null;
}[] = ([
  ["Forester", "Forestry"],
  ["Carpenter", "Carpentry"],
  ["Mason", "Masonry"],
  ["Miner", "Mining"],
  ["Smith", "Smithing"],
  ["Scholar", "Scholar"],
  ["Leatherworker", "Leatherworking"],
  ["Hunter", "Hunting"],
  ["Tailor", "Tailoring"],
  ["Farmer", "Farming"],
  ["Fisher", "Fishing"],
  ["Cook", "Cooking"],
  ["Forager", "Foraging"],
] as const).map(([title, skillName], offset) => ({
  title,
  skillName,
  skill: 2 + offset,
  power: 34 + offset,
  speed: 21 + offset,
  chance: offset === 11 ? null : 55 + (offset === 12 ? 11 : offset),
  crit: offset === 11 ? null : 67 + (offset === 12 ? 11 : offset),
}));
professions.push(
  {
    title: "Builder",
    skillName: "Construction",
    skill: 15,
    power: Stat.ConstructionPower,
    speed: null,
    chance: null,
    crit: null,
  },
  { title: "Hexite Gatherer", skillName: "Hexite Gathering", skill: 22, power: 79, speed: 80, chance: 81, crit: 82 },
);
professions.sort((a, b) => compareSkillNames(a.skillName, b.skillName));

const powerDescription =
  "Profession power bonus plus equipped tool power. An empty tool slot uses the game's no-tool power of 1.";

export function professionStatGroups(
  values: (number | null)[],
  tools: Toolbelt | null | undefined,
  metadata: ToolPowers | null | undefined,
): Group[] {
  return professions.flatMap(({ title, skill, power, speed, chance, crit }) => {
    const read = (id: number | null) => (id === null ? null : (values[id] ?? null));
    const bonus = read(power);
    if ([bonus, read(speed), read(chance), read(crit)].every((value) => value === null)) return [];
    const slot = metadata?.slots_by_skill[skill];
    const pocket = slot === undefined ? undefined : tools?.pockets[slot];
    const toolPower = !pocket
      ? null
      : !pocket.contents
        ? 1
        : pocket.contents.item_type === "Item"
          ? (metadata?.power_by_item[pocket.contents.item_id] ?? null)
          : null;
    const rows = [row("Total Power", sum(bonus, toolPower), "points", powerDescription)];
    if (speed !== null) rows.push(row("Speed", read(speed), "bonus"));
    if (chance !== null) rows.push(row("Crit Chance", read(chance), "percent"));
    // The panel displays the crit bonus (0.50), not the action multiplier (1.50×).
    if (crit !== null) rows.push(row("Crit Multiplier", read(crit), "decimal"));
    return [{ title, rows }];
  });
}

export function createDetailedStatFormatter(locale?: string) {
  const points = new Intl.NumberFormat(locale, { maximumFractionDigits: 0 });
  const percent = new Intl.NumberFormat(locale, { style: "percent", maximumFractionDigits: 0 });
  const bonus = new Intl.NumberFormat(locale, {
    style: "percent",
    maximumFractionDigits: 0,
    signDisplay: "always",
  });
  const signed = new Intl.NumberFormat(locale, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
    signDisplay: "always",
  });
  const decimal = new Intl.NumberFormat(locale, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
  return ({ value, unit }: Row) => {
    if (value === null) return "—";
    switch (unit) {
      case "percent":
        return percent.format(value);
      case "bonus":
        return bonus.format(Math.round((value - 1) * 100) / 100 || 0);
      case "regen":
        return `${signed.format(value)} / second`;
      case "decimal":
        return decimal.format(value);
      case "points":
        return points.format(value);
    }
  };
}
