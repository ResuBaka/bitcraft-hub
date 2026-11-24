import type { AlertType } from "./accountOverviewAlerts";

type Request = { player?: string; type?: AlertType; preview?: boolean };
type Clip = { src: string; requests: Request[] };
type Options = {
  volume: () => number;
  allowed: (player: string, type: AlertType) => boolean;
  sound: (player?: string, type?: AlertType) => string | undefined;
  notice: (message: string, retry?: () => void) => void;
  started?: () => void;
  createAudio?: () => HTMLAudioElement;
};

// Keep one media element so browsers with per-element autoplay permission
// can reuse a successful user-initiated playback for later live alerts.
export function createAlertSoundPlayback(options: Options) {
  let audio: HTMLAudioElement | undefined;
  let current: Clip | undefined;
  let blocked = false;
  let disposed = false;
  let queue: Clip[] = [];

  const allowed = (r: Request) =>
    options.volume() > 0 && (r.preview || !r.type || options.allowed(r.player!, r.type));

  function advance() {
    if (disposed || blocked || current) return;
    while (queue.length) {
      const clip = queue.shift()!;
      if (!clip.requests.some((r) => allowed(r) && options.sound(r.player, r.type) === clip.src))
        continue;
      current = clip;
      audio ??= options.createAudio?.() ?? new Audio();
      const finish = () => {
        if (current !== clip) return;
        current = undefined;
        advance();
      };
      const fail = (error?: { name?: string }) => {
        if (current !== clip) return;
        if (error?.name === "NotAllowedError") {
          current = undefined;
          blocked = true;
          queue.unshift(clip);
          options.notice("The browser blocked automatic alert sounds in this tab.", () => {
            blocked = false;
            advance(); // Called directly by the user's click.
          });
        } else {
          options.notice("The sound could not be played. Try another clip in Alerts.");
          finish();
        }
      };
      audio.onended = finish;
      audio.onerror = () => fail();
      audio.volume = options.volume() / 100;
      audio.src = clip.src;
      void audio.play().then(() => {
        if (current === clip) options.started?.();
      }, fail);
      return;
    }
  }

  function stopCurrent() {
    current = undefined;
    audio?.pause();
  }

  return {
    enqueue(player: string, type: AlertType) {
      const request = { player, type };
      if (disposed || !allowed(request)) return;
      const src = options.sound(player, type);
      if (!src) return;
      // Match the resolved file, including a shared global fallback. Keep the
      // contributing scopes so muting one player doesn't silence another.
      const clip = current?.src === src ? current : queue.find((c) => c.src === src);
      if (clip) {
        if (!clip.requests.some((r) => r.player === player && r.type === type))
          clip.requests.push(request);
        return;
      }
      queue.push({ src, requests: [request] });
      advance();
    },
    test(player?: string, type?: AlertType) {
      if (disposed) return;
      if (!options.volume()) {
        options.notice("Raise Alert volume above 0% to test the sound.");
        return;
      }
      const src = options.sound(player, type);
      if (!src) {
        options.notice("Choose an uploaded sound first.");
        return;
      }
      blocked = false;
      stopCurrent();
      queue = queue.filter((c) => c.src !== src);
      queue.unshift({ src, requests: [{ player, type, preview: true }] });
      advance();
    },
    update() {
      for (const clip of queue) clip.requests = clip.requests.filter(allowed);
      queue = queue.filter((clip) => clip.requests.length);
      if (audio) audio.volume = options.volume() / 100;
      if (current) {
        current.requests = current.requests.filter(allowed);
        if (!current.requests.length) stopCurrent();
      }
      if (!queue.length) blocked = false;
      advance();
    },
    dispose() {
      disposed = true;
      queue = [];
      stopCurrent();
      if (audio) {
        audio.onended = null;
        audio.onerror = null;
      }
    },
  };
}
