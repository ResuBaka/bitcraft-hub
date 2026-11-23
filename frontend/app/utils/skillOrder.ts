export const primarySkills = [
  "Forestry", "Carpentry", "Farming", "Foraging", "Tailoring", "Mining",
  "Smithing", "Masonry", "Scholar", "Hunting", "Leatherworking", "Fishing",
] as const;

export const secondarySkills = [
  "Cooking", "Construction", "Slayer", "Merchanting", "Sailing", "Taming",
] as const;

export const skillOrder = [...primarySkills, ...secondarySkills];
const displayOrder = [
  "Professions", "Skills", "Experience", "Level", "Experience Per Hour", "Time Played", "Time Online",
  ...skillOrder,
];
const positions = new Map<string, number>(displayOrder.map((name, index) => [name, index]));

export function compareSkillNames(a: string, b: string): number {
  return (positions.get(a) ?? positions.size) - (positions.get(b) ?? positions.size)
    || (a < b ? -1 : a > b ? 1 : 0);
}
