import { describe, expect, test } from "bun:test";
import {
  craftClaimKey,
  craftClaimOptions,
  craftMatchesClaims,
} from "../app/utils/craftClaimFilter";

const claim = { building_context: { claim_id: "42", claim_name: "Home" } };
const unclaimed = { building_context: { claim_id: null, claim_name: null } };
const unknown = { building_context: null };

describe("craft claim filters", () => {
  test("all locations are shown when no filter is selected", () => {
    expect(craftMatchesClaims(claim, [])).toBe(true);
    expect(craftMatchesClaims(unclaimed, [])).toBe(true);
    expect(craftMatchesClaims(unknown, [])).toBe(true);
  });

  test("selected claims and unclaimed or unknown locations are distinct", () => {
    expect(craftClaimKey(claim)).toBe("claim:42");
    expect(craftMatchesClaims(claim, ["claim:42"])).toBe(true);
    expect(craftMatchesClaims(unclaimed, ["claim:42"])).toBe(false);
    expect(craftMatchesClaims(unclaimed, ["unclaimed"])).toBe(true);
    expect(craftMatchesClaims(unknown, ["unknown"])).toBe(true);
  });

  test("retains saved filters even when there are no current projects there", () => {
    expect(craftClaimOptions([claim, claim], ["claim:9"])).toEqual([
      { value: "claim:9", label: "Claim #9" },
      { value: "claim:42", label: "Home" },
    ]);
  });
});
