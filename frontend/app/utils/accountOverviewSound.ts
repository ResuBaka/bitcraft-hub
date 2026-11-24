import type { AlertType } from "./accountOverviewAlerts";

export type AlertSound = { id: string; name: string; url: string };
export const soundPrefix = "bitcraft-hub@overview-sound:";
export type SoundChoices = Record<string, string>;
export const soundScope = (player?: string, type?: AlertType) =>
  type ? `type:${type}` : player ? `player:${player}` : "global";

export function readSoundLibrary(): AlertSound[] {
  const clips: AlertSound[] = [];
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (!key?.startsWith(soundPrefix)) continue;
      try {
        const clip = JSON.parse(localStorage.getItem(key) ?? "null");
        if (typeof clip?.name === "string" && /^data:audio\/[\w.+-]+;base64,/.test(clip.url))
          clips.push({ id: key.slice(soundPrefix.length), name: clip.name, url: clip.url });
      } catch {
        /* Ignore an unreadable clip without hiding the rest of the library. */
      }
    }
  } catch {
    /* Browser storage may be unavailable. */
  }
  const sorted = clips.sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
  const names = new Set<string>();
  return sorted.map((clip) => {
    let name = clip.name;
    for (let n = 2; names.has(name); n++) name = `${clip.name} (${n})`;
    names.add(name);
    return { ...clip, name };
  });
}

export function selectedClip(
  clips: AlertSound[],
  choices: SoundChoices,
  player?: string,
  type?: AlertType,
) {
  // Older uploads used "global" or the player ID as the storage key. Keep
  // their original assignments without making a second copy of the audio.
  const id = choices[soundScope(player, type)] ?? (type ? "" : (player ?? "global"));
  return clips.find((clip) => clip.id === id);
}

export function resolveAlertSound(
  clips: AlertSound[],
  choices: SoundChoices,
  player?: string,
  type?: AlertType,
) {
  return (
    (type ? selectedClip(clips, choices, undefined, type) : undefined) ??
    (player ? selectedClip(clips, choices, player) : undefined) ??
    selectedClip(clips, choices)
  );
}

export async function saveAlertSound(file: File): Promise<void> {
  if (!/^audio\/[\w.+-]+$/i.test(file.type))
    throw new Error("Choose an audio file, such as MP3 or WAV.");
  if (file.size > 512 * 1024) throw new Error("Choose a sound smaller than 512 KB.");
  let duration: number;
  try {
    const decoder = new OfflineAudioContext(1, 1, 8000);
    duration = (await decoder.decodeAudioData(await file.arrayBuffer())).duration;
  } catch {
    throw new Error("This browser could not read that audio file.");
  }
  if (!(duration > 0 && duration <= 10)) throw new Error("Choose a sound up to 10 seconds long.");

  const url = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("Could not read the file. Try choosing it again."));
    reader.readAsDataURL(file);
  });
  const clips = readSoundLibrary();
  if (clips.some((clip) => clip.url === url)) return;
  let name = file.name;
  for (let i = 2; clips.some((clip) => clip.name === name); i++) name = `${file.name} (${i})`;
  const id = `clip:${Array.from(crypto.getRandomValues(new Uint32Array(4))).join("-")}`;
  try {
    localStorage.setItem(soundPrefix + id, JSON.stringify({ name, url }));
  } catch {
    throw new Error(
      "Could not save the sound in this browser. Try a smaller file or remove another sound.",
    );
  }
}

export function renameAlertSound(id: string, newName: string) {
  const name = newName.trim();
  if (!name) throw new Error("Enter a name for the sound.");
  const clips = readSoundLibrary();
  const clip = clips.find((item) => item.id === id);
  if (!clip) throw new Error("This sound is no longer in the library.");
  if (name === clip.name) return;
  if (clips.some((item) => item.name === name))
    throw new Error("Another sound already has that name.");
  try {
    localStorage.setItem(soundPrefix + id, JSON.stringify({ name, url: clip.url }));
  } catch {
    throw new Error("Could not save the new name in this browser.");
  }
}

export function removeAlertSound(id: string) {
  localStorage.removeItem(soundPrefix + id);
}
