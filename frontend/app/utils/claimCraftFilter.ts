type CraftVisibility = { kind: "Active" | "Passive"; status: string; public: boolean | null };

export function isCraftVisible(craft: CraftVisibility, showAllActive: boolean, showFinishedActive: boolean, showPassive: boolean): boolean {
  if (craft.kind === "Passive") return showPassive;
  if (!showAllActive && craft.public !== true) return false;
  return showFinishedActive || craft.kind !== "Active" || craft.status !== "Complete";
}
