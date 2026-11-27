import { expect, test } from "bun:test";
import {
  readAtlasMonitor,
  trackedAtlasPlayers,
  type MonitorTarget,
} from "../app/utils/atlasMonitor";

const player = (id: string, name: string): MonitorTarget => ({ kind: "player", id, name });
const claim = (id: string): MonitorTarget => ({ kind: "claim", id, name: `Claim ${id}` });
const resource = (id: string): MonitorTarget => ({ kind: "resource", id, name: `Resource ${id}` });

test("stored favorites are separate from active tracking and retain exact IDs", () => {
  const favorite = player("18446744073709551615", "Alice");
  expect(
    readAtlasMonitor(
      JSON.stringify({
        tracking: [claim("9"), claim("9")],
        favorites: [favorite, player("18446744073709551616", "Invalid"), player("0", "Invalid")],
      }),
    ),
  ).toEqual({ tracking: [claim("9")], favorites: [favorite], playerColors: {} });
  expect(readAtlasMonitor("not JSON")).toEqual({ tracking: [], favorites: [], playerColors: {} });
});

test("saved player colors preserve exact IDs and accept only valid hex overrides", () => {
  const preferences = readAtlasMonitor(
    '{"playerColors":{"18446744073709551615":"#ABC","2":"red","3":"#ffd27a","0":"#123456"}}',
  );
  expect(preferences.playerColors).toEqual({ "18446744073709551615": "#aabbcc" });
});

test("claim rosters and individually tracked players are deduplicated", () => {
  const roster = {
    "9": [
      { id: "1", name: "Old name" },
      { id: "2", name: "Bea" },
    ],
  };
  const tracked = [player("1", "Alice"), claim("9"), claim("10")];
  expect([...trackedAtlasPlayers(tracked, { ...roster, "10": [{ id: "2", name: "" }] })]).toEqual([
    ["1", "Alice"],
    ["2", "Bea"],
  ]);
  expect([...trackedAtlasPlayers(tracked, { "9": [{ id: "3", name: "Cora" }] })]).toEqual([
    ["3", "Cora"],
    ["1", "Alice"],
  ]);
});

test("resource favorites support signed resource IDs without becoming player tracking targets", () => {
  const preferences = readAtlasMonitor(
    JSON.stringify({
      tracking: [player("2", "Bea"), resource("2")],
      favorites: [
        player("2", "Bea"),
        resource("2"),
        resource("-2147483648"),
        resource("2147483648"),
        resource("02"),
      ],
    }),
  );
  expect(preferences.tracking).toEqual([player("2", "Bea")]);
  expect(preferences.favorites).toEqual([
    player("2", "Bea"),
    resource("2"),
    resource("-2147483648"),
  ]);
});
