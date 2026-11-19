import { describe, expect, test } from "bun:test";
import {
  ActivityLinger,
  activityCraftProgress,
  currentPlayerActivity,
} from "../app/utils/playerActivity";
import type { PlayerActivitySnapshot } from "../app/types/PlayerActivitySnapshot";
import type { PlayerCraft } from "../app/types/PlayerCraft";

const snapshot: PlayerActivitySnapshot = {
  player_id: "42",
  available: true,
  signed_in: true,
  walking: false,
  server_time_ms: 1000,
  actions: [
    {
      action_type: "None",
      successful: true,
      consumed: false,
      start_ms: 1000,
      duration_ms: 0,
      recipe_id: null,
    },
  ],
};
const action = (action_type: string, recipe_id: number | null = null) => ({
  action_type,
  recipe_id,
  successful: true,
  consumed: false,
  start_ms: 1000,
  duration_ms: 2000,
});

test("keeps the last action briefly, but switches immediately to another action or offline", () => {
  const linger = new ActivityLinger(3000);
  const craft = currentPlayerActivity({ ...snapshot, actions: [action("Craft", 1)] }, 1500);
  const idle = currentPlayerActivity(snapshot, 1500);
  const move = currentPlayerActivity({ ...snapshot, actions: [action("PlayerMove")] }, 1500);
  const offline = currentPlayerActivity({ ...snapshot, signed_in: false }, 1500);
  expect(linger.update(craft, 1500).label).toBe("Crafting");
  expect(linger.update(idle, 4499).label).toBe("Crafting");
  expect(linger.update(idle, 4500).label).toBe("Idle");
  linger.update(craft, 5000);
  expect(linger.update(move, 5100).label).toBe("Running");
  expect(linger.update(offline, 5200).label).toBe("Offline");
  expect(linger.update(idle, 5300).label).toBe("Idle");
});

test("only shows progress for a unique matching building and recipe", () => {
  const activity = currentPlayerActivity(
    {
      ...snapshot,
      actions: [{ ...action("Craft", 7), target_id: "9007199254740993" }],
    },
    1500,
  );
  const craft = {
    kind: "Active",
    building_id: "9007199254740993",
    recipe_id: 7,
    progress: 5,
    progress_total: 20,
  } as PlayerCraft;
  expect(activityCraftProgress(activity, [craft])).toEqual({ progress: 5, total: 20, percent: 25 });
  expect(activityCraftProgress(activity, [])).toBeNull();
  expect(activityCraftProgress(activity, [craft, craft])).toBeNull();
  expect(activityCraftProgress(activity, [{ ...craft, building_id: "1" }])).toBeNull();
  expect(activityCraftProgress(activity, [{ ...craft, progress_total: null }])).toBeNull();
});

describe("player activity", () => {
  test("distinguishes unknown, offline and idle", () => {
    expect(currentPlayerActivity({ ...snapshot, available: false }, 1500).label).toBe(
      "Activity unavailable",
    );
    expect(currentPlayerActivity({ ...snapshot, actions: null }, 1500).label).toBe(
      "Activity unavailable",
    );
    expect(currentPlayerActivity({ ...snapshot, signed_in: false }, 1500).label).toBe("Offline");
    expect(currentPlayerActivity(snapshot, 1500).label).toBe("Idle");
  });
  test("shows crafting and gathering recipes instead of movement", () => {
    expect(
      currentPlayerActivity(
        { ...snapshot, actions: [action("PlayerMove"), action("Craft", 15)] },
        1500,
      ),
    ).toMatchObject({ label: "Crafting", recipeKind: "craft", recipeId: 15 });
    expect(
      currentPlayerActivity({ ...snapshot, actions: [action("Extract", 23)] }, 1500),
    ).toMatchObject({ label: "Gathering", recipeKind: "gather", recipeId: 23 });
  });
  test("expires completed or failed actions and distinguishes walking from running", () => {
    expect(
      currentPlayerActivity({ ...snapshot, actions: [action("PlayerMove")] }, 1500).label,
    ).toBe("Running");
    expect(
      currentPlayerActivity({ ...snapshot, walking: true, actions: [action("PlayerMove")] }, 1500)
        .label,
    ).toBe("Walking");
    expect(
      currentPlayerActivity({ ...snapshot, walking: null, actions: [action("PlayerMove")] }, 1500)
        .label,
    ).toBe("Moving");
    expect(currentPlayerActivity({ ...snapshot, actions: [action("Craft", 1)] }, 3000).label).toBe(
      "Idle",
    );
    expect(
      currentPlayerActivity(
        { ...snapshot, actions: [{ ...action("Extract"), consumed: true }] },
        1500,
      ).label,
    ).toBe("Idle");
    expect(
      currentPlayerActivity(
        { ...snapshot, actions: [{ ...action("Craft"), start_ms: 10_000 }] },
        1500,
      ).label,
    ).toBe("Idle");
    expect(
      currentPlayerActivity(
        { ...snapshot, actions: [{ ...action("Extract"), successful: false }] },
        1500,
      ).label,
    ).toBe("Idle");
  });
});
