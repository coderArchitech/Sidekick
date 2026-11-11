import { useId, useState } from "react";
import type { Profile } from "@/ipc";

interface Props {
  profile: Profile;
  onSave: (profile: Profile) => Promise<{ saved: Profile } | { error: string }>;
}

/**
 * Settings → Profile (PRD QA-2, LIS-5a). Sidekick listens for these names to
 * spot questions addressed to you, and tells speech-to-text to expect them.
 */
export function ProfileSection({ profile, onSave }: Props) {
  const [name, setName] = useState(profile.name);
  const [nicknames, setNicknames] = useState(profile.nicknames);
  const [newNick, setNewNick] = useState("");
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const nameId = useId();
  const nickId = useId();

  function add() {
    const n = newNick.trim();
    if (n && !nicknames.some((x) => x.toLowerCase() === n.toLowerCase()))
      setNicknames([...nicknames, n]);
    setNewNick("");
  }

  const input = "field text-sm";
  const button = "btn btn-secondary";

  return (
    <section aria-labelledby="profile-heading" className="card p-4">
      <h2 id="profile-heading" className="heading text-xl">
        Profile
      </h2>
      <p className="mb-3 text-sm text-fg-muted">
        Questions that use these names are answered first, even with Auto-answer off.
      </p>
      <label htmlFor={nameId} className="block text-sm text-fg-2">
        Your name
      </label>
      <input
        id={nameId}
        value={name}
        onChange={(e) => setName(e.target.value)}
        className={`mt-1 ${input}`}
      />

      <h3 className="mt-3 text-sm text-fg-2">Nicknames</h3>
      {nicknames.length === 0 ? (
        <p className="text-sm text-fg-muted">None yet.</p>
      ) : (
        <ul aria-label="Nicknames" className="mt-1 flex flex-wrap gap-2">
          {nicknames.map((n) => (
            <li
              key={n}
              className="flex items-center gap-1 rounded-full border border-line-strong px-2 py-0.5 text-sm"
            >
              {n}
              <button
                type="button"
                aria-label={`Remove ${n}`}
                className="text-fg-muted hover:text-fg"
                onClick={() => setNicknames(nicknames.filter((x) => x !== n))}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      )}
      <form
        className="mt-2 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          add();
        }}
      >
        <label htmlFor={nickId} className="sr-only">
          New nickname
        </label>
        <input
          id={nickId}
          value={newNick}
          placeholder="Add a nickname"
          onChange={(e) => setNewNick(e.target.value)}
          className={input}
        />
        <button type="submit" className={button} disabled={!newNick.trim()}>
          Add
        </button>
      </form>
      <button
        type="button"
        className="btn btn-primary mt-3"
        onClick={() =>
          void onSave({ name, nicknames }).then((r) => {
            if ("error" in r) {
              setMessage({ ok: false, text: r.error });
            } else {
              setName(r.saved.name);
              setNicknames(r.saved.nicknames);
              setMessage({ ok: true, text: "Profile saved." });
            }
          })
        }
      >
        Save profile
      </button>
      {message && (
        <p
          role={message.ok ? "status" : "alert"}
          className={`mt-2 text-sm ${message.ok ? "text-success" : "text-danger"}`}
        >
          {message.text}
        </p>
      )}
    </section>
  );
}
