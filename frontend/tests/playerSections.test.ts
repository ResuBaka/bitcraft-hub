import { describe, expect, test } from "bun:test";
import { getFavoritePlayerSections, getPlayerSection, insertPlayerSectionFavorite, movePlayerSectionFavorite, playerSections, togglePlayerSectionFavorite } from "../app/utils/playerSections";

describe("player section routes", () => {
  test("uses the base player URL for overview and resolves each unique section slug", () => {
    expect(getPlayerSection(undefined)?.value).toBe("overview");
    expect(getPlayerSection("")?.value).toBe("overview");
    expect(getPlayerSection("stats")?.value).toBe("stats");
    expect(getPlayerSection("xp-history")?.value).toBe("xp_history");
    expect(getPlayerSection("traveler-tasks")?.value).toBe("traveler_tasks");
    for (const section of playerSections) {
      expect(getPlayerSection(section.slug)).toBe(section);
    }
  });

  test("rejects unknown and multi-segment section params", () => {
    expect(getPlayerSection("typo")).toBeUndefined();
    expect(getPlayerSection(["stats", "other"])).toBeUndefined();
  });
});

describe("player section favorites", () => {
  test("pins and unpins sections without changing the original list", () => {
    const favorites = ["skills"];
    expect(togglePlayerSectionFavorite(favorites, "houses")).toEqual(["skills", "houses"]);
    expect(togglePlayerSectionFavorite(favorites, "skills")).toEqual([]);
    expect(favorites).toEqual(["skills"]);
  });

  test("shows only favorites in the saved order without changing the original sections", () => {
    const sections = ["overview", "skills", "inventories", "houses"].map((value) => ({ value }));
    expect(getFavoritePlayerSections(sections, ["houses", "skills", "unknown"]).map((section) => section.value))
      .toEqual(["houses", "skills"]);
    expect(sections[0]?.value).toBe("overview");
  });

  test("moves favorites to the dropped position", () => {
    const favorites = ["skills", "houses", "overview"];
    expect(movePlayerSectionFavorite(favorites, "houses", "skills")).toEqual(["houses", "skills", "overview"]);
    expect(movePlayerSectionFavorite(favorites, "skills", "overview")).toEqual(["houses", "overview", "skills"]);
    expect(movePlayerSectionFavorite(favorites, "skills", "skills")).toEqual(favorites);
    expect(movePlayerSectionFavorite(favorites, "missing", "skills")).toEqual(favorites);
    expect(movePlayerSectionFavorite(favorites, "skills", "missing")).toEqual(favorites);
    expect(favorites).toEqual(["skills", "houses", "overview"]);
  });

  test("inserts before or after the previewed favorite", () => {
    const favorites = ["skills", "houses", "overview", "crafting"];
    expect(insertPlayerSectionFavorite(favorites, "skills", "overview", "before")).toEqual(["houses", "skills", "overview", "crafting"]);
    expect(insertPlayerSectionFavorite(favorites, "skills", "overview", "after")).toEqual(["houses", "overview", "skills", "crafting"]);
    expect(insertPlayerSectionFavorite(favorites, "crafting", "houses", "after")).toEqual(["skills", "houses", "crafting", "overview"]);
    expect(insertPlayerSectionFavorite(favorites, "skills", "skills", "before")).toEqual(favorites);
    expect(insertPlayerSectionFavorite(favorites, "missing", "skills", "before")).toEqual(favorites);
  });
});
