import { describe, expect, test } from "bun:test";
import type { ClaimTechDesc } from "../app/types/ClaimTechDesc";
import { getClaimMaxSupplies } from "../app/utils/claimSupplies";

const upgrade = (id: number, tech_type: ClaimTechDesc["tech_type"], supplies: number) =>
  ({ id, tech_type, supplies }) as ClaimTechDesc;

const upgrades = [
  upgrade(1, "StarterTech", 12000),
  upgrade(2, "MaxSupplies", 24000),
  upgrade(3, "MaxSupplies", 36000),
  upgrade(4, "MemberCount", 99999),
];

describe("claim supply capacity", () => {
  test("uses starter capacity when starter technology is learned", () => {
    expect(getClaimMaxSupplies(upgrades, [1])).toBe(12000);
  });

  test("uses the highest learned supply capacity, ignoring unlearned research", () => {
    expect(getClaimMaxSupplies(upgrades, [1, 2])).toBe(24000);
    expect(getClaimMaxSupplies(upgrades, [3, 2])).toBe(36000);
  });

  test("includes learned technologies of any type", () => {
    expect(getClaimMaxSupplies(upgrades, [1, 4])).toBe(99999);
  });

  test("returns unknown when learned research or its descriptions are missing", () => {
    expect(getClaimMaxSupplies([], [])).toBeNull();
    expect(getClaimMaxSupplies(upgrades, [])).toBeNull();
    expect(getClaimMaxSupplies(upgrades, [1, 999])).toBeNull();
  });
});
