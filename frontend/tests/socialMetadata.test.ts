import { describe, expect, test } from "bun:test";
import { socialUrls } from "../app/utils/socialMetadata";

describe("social metadata URLs", () => {
  test("uses the configured public origin and removes query and fragment", () => {
    expect(socialUrls("http://internal:3000", "https://hub.example", "/claims/123?tab=crafting#x", "/Icons/SkillIconANY.png")).toEqual({
      url: "https://hub.example/claims/123", image: "https://hub.example/Icons/SkillIconANY.png",
    });
  });
  test("falls back to the request origin and supports external or absent images", () => {
    expect(socialUrls("https://hub.example", "", "/players/42", "https://cdn.example/social.png")).toEqual({ url: "https://hub.example/players/42", image: "https://cdn.example/social.png" });
    expect(socialUrls("https://hub.example", "", "/players/42", "").image).toBeUndefined();
  });
  test("handles an invalid base without throwing", () => {
    expect(socialUrls("invalid", "", "/claims/1", "image.png")).toEqual({ url: undefined, image: undefined });
  });
});
