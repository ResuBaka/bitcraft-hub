import { expect, test } from "bun:test";
import { createAlertSoundPlayback } from "../app/utils/alertSoundPlayback";
import { resolveAlertSound, type SoundChoices } from "../app/utils/accountOverviewSound";
import type { AlertType } from "../app/utils/accountOverviewAlerts";

function fixture(
  sound: (player?: string, type?: AlertType) => string | undefined = (player) => player ?? "global",
) {
  const played: string[] = [];
  const notices: string[] = [];
  const state = { blocked: false, muted: false, volume: 60 };
  const mutedPlayers = new Set<string>();
  let retry: (() => void) | undefined;
  const audio = {
    src: "",
    volume: 1,
    onended: null as (() => void) | null,
    play: async () => {
      if (state.blocked) throw new DOMException("Click required", "NotAllowedError");
      played.push(audio.src);
    },
    pause: () => {},
  };
  const playback = createAlertSoundPlayback({
    volume: () => state.volume,
    allowed: (player) => !state.muted && !mutedPlayers.has(player),
    sound,
    createAudio: () => audio as unknown as HTMLAudioElement,
    notice: (message, action) => {
      notices.push(message);
      retry = action;
    },
  });
  return { playback, played, notices, state, mutedPlayers, audio, retry: () => retry?.() };
}

test("type assignments use library clips for live playback and previews, with legacy defaults", () => {
  const clips = ["global", "42", "clip:chime"].map((id) => ({ id, name: id, url: `${id}.wav` }));
  const choices: SoundChoices = {
    "type:low-health": "clip:chime",
    "type:food-expired": "clip:chime",
  };
  const resolve = (p?: string, t?: AlertType) => resolveAlertSound(clips, choices, p, t)?.url;
  const { playback, played, audio, state } = fixture(resolve);
  playback.enqueue("42", "full-stamina");
  playback.enqueue("42", "low-health");
  playback.enqueue("99", "food-expired");
  audio.onended!();
  audio.onended!();
  expect(played).toEqual(["42.wav", "clip:chime.wav"]);
  state.muted = true;
  playback.test("42", "food-expired");
  expect(played.at(-1)).toBe("clip:chime.wav");
  expect(played).toHaveLength(3);
  choices["type:food-expired"] = "";
  expect(resolve("42", "food-expired")).toBe("42.wav");
  choices["player:42"] = "";
  expect(resolve("42", "food-expired")).toBe("global.wav");
  clips.pop();
  expect(resolve("99", "low-health")).toBe("global.wav");
  choices.global = "";
  expect(resolve("99", "low-health")).toBeUndefined();
});

test("overlapping alerts play each distinct file once, even across players and types", () => {
  const { playback, audio, played, mutedPlayers } = fixture((player) =>
    player === "a" || player === "b" ? "global.wav" : "custom.wav",
  );
  playback.enqueue("a", "full-stamina");
  playback.enqueue("b", "activity-stopped"); // Same as playing clip
  playback.enqueue("c", "low-health");
  playback.enqueue("d", "food-expired"); // Same as queued clip
  mutedPlayers.add("c");
  playback.update();
  expect(played).toEqual(["global.wav"]);
  audio.onended!();
  expect(played).toEqual(["global.wav", "custom.wav"]);
  audio.onended!();
  expect(played).toEqual(["global.wav", "custom.wav"]);
  playback.enqueue("b", "activity-stopped"); // A later, non-overlapping alert
  expect(played).toEqual(["global.wav", "custom.wav", "global.wav"]);
});

test("an idle sound waits for the current clip and respects subsequent mutes", () => {
  const { playback, audio, played, state } = fixture();
  playback.enqueue("a", "full-stamina");
  playback.enqueue("b", "activity-stopped");
  expect(played).toEqual(["a"]);
  audio.onended!();
  expect(played).toEqual(["a", "b"]);
  expect(audio.volume).toBe(0.6);
  playback.enqueue("c", "activity-stopped");
  state.muted = true;
  playback.update();
  audio.onended!();
  expect(played).toEqual(["a", "b"]);
});

test("autoplay rejection can be retried once without another alert event", async () => {
  const { playback, played, retry, notices, state, audio } = fixture();
  state.blocked = true;
  playback.enqueue("a", "activity-stopped");
  await Promise.resolve();
  expect(played).toEqual([]);
  expect(notices[0]).toContain("blocked");
  state.blocked = false;
  retry();
  expect(played).toEqual(["a"]);
  audio.onended!();
  expect(played).toEqual(["a"]);
});

test("settings test uses the same player as live alerts; leaving drops pending audio", () => {
  const { playback, audio, played } = fixture();
  playback.test();
  playback.enqueue("a", "activity-stopped");
  audio.onended!();
  playback.enqueue("b", "activity-stopped");
  playback.dispose();
  expect(played).toEqual(["global", "a"]);
  expect(audio.onended).toBeNull();
});
