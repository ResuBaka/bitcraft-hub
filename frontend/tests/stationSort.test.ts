import { describe, expect, test } from "bun:test";
import { compareProjectOwners, compareStations } from "../app/utils/stationSort";

describe("station sort", () => {
  test("groups different names with the same function and puts unknown functions last", () => {
    const stations = [
      { id: "1", name: "Basic Smelter", functionTypes: [4] },
      { id: "2", name: "Loom", functionTypes: [9] },
      { id: "3", name: "Advanced Smelter", functionTypes: [4] },
      { id: "4", name: "Unknown", functionTypes: [] },
    ];
    expect(stations.sort(compareStations).map((station) => station.id)).toEqual(["3", "1", "2", "4"]);
  });
  test("sorts multi-function types lexicographically", () => {
    expect(compareStations({ id: "1", name: "Z", functionTypes: [4, 7] }, { id: "2", name: "A", functionTypes: [4, 8] })).toBeLessThan(0);
  });
  test("sorts members before non-members and then by owner", () => {
    const owners = [
      { is_member: false, owner_name: "Alice", owner_id: "1" },
      { is_member: true, owner_name: "Zoe", owner_id: "2" },
      { is_member: true, owner_name: "Bob", owner_id: "3" },
    ];
    expect(owners.sort(compareProjectOwners).map((owner) => owner.owner_id)).toEqual(["3", "2", "1"]);
  });
});
