import { describe, expect, test } from "bun:test";
import { hasSubscribedTopic, WebsocketTopicRegistry } from "../app/utils/websocketTopicRegistry";

describe("websocket topic ownership", () => {
  test("keeps shared topics until the last owner releases them", () => {
    const registry = new WebsocketTopicRegistry();
    expect(registry.add("active", ["player_crafting", "player_crafting.1"])).toEqual([
      "player_crafting",
      "player_crafting.1",
    ]);
    expect(registry.add("passive", ["player_crafting", "player_crafting.1"])).toEqual([]);
    expect(registry.add("other", ["player_crafting", "player_crafting.2"])).toEqual([
      "player_crafting.2",
    ]);
    expect(registry.remove("active", ["player_crafting", "player_crafting.1"]).released).toEqual(
      [],
    );
    expect(registry.remove("passive", ["player_crafting", "player_crafting.1"]).released).toEqual([
      "player_crafting.1",
    ]);
    expect(registry.remove("other", ["player_crafting", "player_crafting.2"]).released).toEqual([
      "player_crafting",
      "player_crafting.2",
    ]);
  });

  test("partial changes preserve the handler's remaining topics", () => {
    const registry = new WebsocketTopicRegistry();
    registry.add("owner", ["one", "two"]);
    expect(registry.remove("owner", ["one"])).toEqual({ released: ["one"], hasOwner: true });
    expect(registry.add("owner", ["two", "three"])).toEqual(["three"]);
    expect(registry.remove("owner", ["two", "three"])).toEqual({
      released: ["two", "three"],
      hasOwner: false,
    });
  });

  test("permanent and empty subscriptions are handled", () => {
    const registry = new WebsocketTopicRegistry();
    expect(registry.addPermanent(["global"])).toEqual(["global"]);
    expect(registry.add("listener", ["global"])).toEqual([]);
    expect(registry.remove("listener", ["global"]).released).toEqual([]);
    registry.add("ack", []);
    expect(registry.remove("ack", [])).toEqual({ released: [], hasOwner: false });
  });

  test("recognizes canonical and legacy subscription acknowledgements", () => {
    expect(hasSubscribedTopic(["player_crafting.42"], "player_crafting", "42")).toBe(true);
    expect(hasSubscribedTopic(["player_crafting.Some(42)"], "player_crafting", "42")).toBe(true);
    expect(hasSubscribedTopic(["player_crafting.Some(420)"], "player_crafting", "42")).toBe(false);
  });
});
