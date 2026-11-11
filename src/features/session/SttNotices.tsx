import type { Stream, SttState } from "@/ipc";

const WHO: Record<Stream, string> = {
  mic: "your microphone",
  system: "other participants",
};

const isMac = () => typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);

/** What to tell the user about one stream's speech-to-text state, if anything. */
export function sttNotice(
  stream: Stream,
  state: SttState | undefined,
): { text: string; alert: boolean } | null {
  switch (state) {
    case "silent":
      return {
        alert: true,
        text: isMac()
          ? "Sidekick isn't hearing your microphone: it only gets silence. Allow Sidekick in System Settings → Privacy & Security → Microphone, then quit and reopen Sidekick."
          : "Sidekick isn't hearing your microphone: it only gets silence. Allow microphone access for Sidekick in Settings → Privacy & security → Microphone, then restart Sidekick.",
      };
    case "failed":
      return {
        alert: true,
        text: `Speech-to-text stopped for ${WHO[stream]}. Check the key in Settings → Speech-to-text, then end the session and start a new one.`,
      };
    case "reconnecting":
      return { alert: false, text: `Reconnecting speech-to-text for ${WHO[stream]}…` };
    default:
      return null;
  }
}

/** Speech-to-text problems for the live session, under the session controls. */
export function SttNotices({ stt }: { stt: Partial<Record<Stream, SttState>> }) {
  const notices = (["mic", "system"] as const)
    .map((s) => sttNotice(s, stt[s]))
    .filter((n) => n !== null);
  return notices.map((n) => (
    <p
      key={n.text}
      role={n.alert ? "alert" : "status"}
      className={n.alert ? "text-danger" : "text-fg-muted"}
    >
      {n.text}
    </p>
  ));
}
