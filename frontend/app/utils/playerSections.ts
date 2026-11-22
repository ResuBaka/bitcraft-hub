export const equipmentPlayerSection = {
  value: "equipment",
  slug: "equipment",
  label: "Equipment",
  icon: "i-lucide-shirt",
} as const;

export function getFavoritePlayerSections<T extends { value: string }>(sections: T[], favorites: string[]): T[] {
  const byValue = new Map(sections.map((section) => [section.value, section]));
  return [...new Set(favorites)].flatMap((value) => {
    const section = byValue.get(value);
    return section ? [section] : [];
  });
}

export function togglePlayerSectionFavorite(favorites: string[], value: string): string[] {
  return favorites.includes(value)
    ? favorites.filter((favorite) => favorite !== value)
    : [...favorites, value];
}

export function movePlayerSectionFavorite(favorites: string[], value: string, targetValue: string): string[] {
  const index = favorites.indexOf(value);
  const target = favorites.indexOf(targetValue);
  if (index < 0 || target < 0 || index === target) return favorites;
  const result = [...favorites];
  result.splice(index, 1);
  result.splice(target, 0, value);
  return result;
}

export function insertPlayerSectionFavorite(
  favorites: string[], value: string, targetValue: string, position: "before" | "after",
): string[] {
  if (!favorites.includes(value) || !favorites.includes(targetValue) || value === targetValue) return favorites;
  const result = favorites.filter((favorite) => favorite !== value);
  const target = result.indexOf(targetValue);
  result.splice(target + (position === "after" ? 1 : 0), 0, value);
  return result;
}
