import { describe, expect, test } from "bun:test";
import {
  ActivityLinger,
  activityCraft,
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
    status: "Active",
    building_id: "9007199254740993",
    recipe_id: 7,
    progress: 5,
    progress_total: 20,
  } as PlayerCraft;
  expect(activityCraftProgress(activityCraft(activity, [craft]))).toEqual({
    progress: 5,
    total: 20,
    percent: 25,
  });
  expect(activityCraft(activity, [])).toBeNull();
  expect(activityCraft(activity, [craft, craft])).toBeNull();
  expect(activityCraft(activity, [{ ...craft, status: "Complete" }, craft])).toBe(craft);
  expect(activityCraft(activity, [{ ...craft, building_id: "1" }])).toBeNull();
  expect(activityCraftProgress({ ...craft, progress_total: null })).toBeNull();
});

test("resolves the owner's exact project even when several projects use the same recipe", () => {
  const target = "9007199254740993";
  const activity = currentPlayerActivity(
    {
      ...snapshot,
      actions: [{ ...action("Craft", 7), target_id: "2", craft_id: target, craft_owner_id: "43" }],
    },
    1500,
  );
  const craft = { kind: "Active", id: target, building_id: "2", recipe_id: 7 } as PlayerCraft;
  const other = { ...craft, id: "9007199254740992" };
  expect(activity.craftOwnerId).toBe("43");
  expect(activityCraft(activity, [other, craft])).toBe(craft);
  expect(activityCraft({ ...activity, recipeId: null }, [other, craft])).toBe(craft);
  expect(activityCraft(activity, [other])).toBeNull();
  expect(activityCraft(activity, [{ ...craft, recipe_id: 8 }])).toBeNull();
  expect(activityCraft(activity, [{ ...craft, kind: "Passive" }])).toBeNull();
});

describe("player activity", () => {
  test.each([
    ["PlayerMove", "Cart", "Handcart", true, "Pulling Handcart"],
    ["DeployableMove", "Cart", "Covered Wagon", false, "Riding Covered Wagon"],
    ["DeployableMove", "Boat", "Fishing Skiff", true, "Sailing Fishing Skiff"],
    ["PlayerMove", "Boat", "Fishing Skiff", false, "Aboard Fishing Skiff"],
    ["PlayerMove", "Mount", "Mountain Goat", true, "Riding Mountain Goat"],
    ["PlayerMove", null, null, true, "Using a deployable"],
  ] as const)("reports %s on %s (%s, driver=%s) as %s", (type, kind, name, driver, label) => {
    const mount = { entity_id: "99", driver, kind, name };
    expect(currentPlayerActivity({ ...snapshot, mount, actions: [action(type)] }, 1500).label).toBe(
      label,
    );
  });

  test("mounting and dismounting clear stale movement; mounted gathering still takes priority", () => {
    const mount = { entity_id: "99", driver: true, kind: "Mount", name: "Mountain Goat" };
    const moving = { ...snapshot, actions: [action("PlayerMove")] };
    const linger = new ActivityLinger();
    linger.update(currentPlayerActivity(moving, 1500), 1500);
    expect(linger.update(currentPlayerActivity({ ...snapshot, mount }, 1501), 1501)).toMatchObject({
      label: "Idle",
      detail: "On Mountain Goat",
    });
    linger.update(currentPlayerActivity({ ...moving, mount }, 1502), 1502);
    expect(linger.update(currentPlayerActivity(snapshot, 1503), 1503).label).toBe("Idle");
    expect(
      currentPlayerActivity(
        {
          ...moving,
          mount,
          actions: [...moving.actions, action("Extract", 23)],
        },
        1504,
      ),
    ).toMatchObject({ label: "Gathering", recipeId: 23 });
  });

  test("recognizes a delayed public-project hit without keeping stale work active", () => {
    const hit = {
      ...action("Craft", 203006),
      start_ms: 2000,
      duration_ms: 1190,
      craft_owner_id: "43",
    };
    const sample = { ...snapshot, server_time_ms: 9296, actions: [hit] };
    expect(currentPlayerActivity(sample, 9296)).toMatchObject({
      label: "Crafting",
      craftOwnerId: "43",
    });
    expect(currentPlayerActivity(sample, 11000).label).toBe("Idle");
    expect(currentPlayerActivity({ ...sample, server_time_ms: 11000 }, 11000).label).toBe("Idle");
    const consumed = { ...hit, consumed: true };
    expect(currentPlayerActivity({ ...sample, actions: [consumed] }, 9296).label).toBe("Crafting");
    expect(
      currentPlayerActivity(
        {
          ...sample,
          actions: [consumed, { ...action("PlayerMove"), start_ms: 9200 }],
        },
        9296,
      ).label,
    ).toBe("Running");
  });
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
        3000,
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
