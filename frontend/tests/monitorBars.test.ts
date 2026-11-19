import { describe, expect, test } from "bun:test";
import { formatFoodTimer, meterValue, summarizeFoodBuffs } from "../app/utils/monitorBars";

describe("monitor bars", () => {
  test("shows food time in minutes until less than a minute remains", () => {
    expect(formatFoodTimer(3661)).toBe("61m");
    expect(formatFoodTimer(119)).toBe("1m");
    expect(formatFoodTimer(60)).toBe("1m");
    expect(formatFoodTimer(59)).toBe("59s");
    expect(formatFoodTimer(0)).toBe("0s");
  });
  test("only calculates a bar with valid current and maximum values", () => {
    expect(meterValue(50, 100)).toBe(50);
    expect(meterValue(200, 100)).toBe(100);
    expect(meterValue(-5, 100)).toBe(0);
    expect(meterValue(20, null)).toBeNull();
    expect(meterValue(null, 100)).toBeNull();
    expect(meterValue(20, 0)).toBeNull();
  });

  test("only includes active food-associated buffs and chooses the next running expiry", () => {
    const result = summarizeFoodBuffs(
      [
        { is_food_associated: false, remaining_seconds: 1, duration: 10, paused: false },
        { is_food_associated: true, remaining_seconds: 40, duration: 100, paused: false },
        { is_food_associated: true, remaining_seconds: 10, duration: 100, paused: true },
        { is_food_associated: true, remaining_seconds: 2, duration: 100, paused: false },
      ],
      5,
    );
    expect(result.count).toBe(2);
    expect(result.next?.remaining).toBe(35);
    expect(result.next?.paused).toBe(false);
  });

  test("keeps paused and permanent buffs without counting down", () => {
    const result = summarizeFoodBuffs(
      [
        { is_food_associated: true, remaining_seconds: 8, duration: 100, paused: true },
        { is_food_associated: true, remaining_seconds: null, duration: -1, paused: false },
        { is_food_associated: null, remaining_seconds: 30, duration: 100, paused: false },
      ],
      50,
    );
    expect(result.count).toBe(2);
    expect(result.next?.remaining).toBe(8);
    expect(result.next?.paused).toBe(true);
  });
});
