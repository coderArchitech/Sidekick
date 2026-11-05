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

  const input = "rounded-md border border-slate-600 bg-slate-950 px-2 py-1 text-sm";
  const button =
    "rounded-md border border-slate-600 px-2.5 py-1 text-sm hover:bg-slate-700 disabled:opacity-50";

  return (
    <section aria-labelledby="profile-heading" className="rounded-lg border border-slate-700 p-4">
      <h2 id="profile-heading" className="text-base font-semibold">
        Profile
      </h2>
      <p className="mb-3 text-sm text-slate-400">
        Questions that use these names are answered first, even with Auto-answer off.
      </p>
      <label htmlFor={nameId} className="block text-sm text-slate-300">
        Your name
      </label>
      <input
        id={nameId}
        value={name}
        onChange={(e) => setName(e.target.value)}
        className={`mt-1 ${input}`}
      />

      <h3 className="mt-3 text-sm text-slate-300">Nicknames</h3>
      {nicknames.length === 0 ? (
        <p className="text-sm text-slate-400">None yet.</p>
      ) : (
        <ul aria-label="Nicknames" className="mt-1 flex flex-wrap gap-2">
          {nicknames.map((n) => (
            <li
              key={n}
              className="flex items-center gap-1 rounded-full border border-slate-600 px-2 py-0.5 text-sm"
            >
              {n}
              <button
                type="button"
                aria-label={`Remove ${n}`}
                className="text-slate-400 hover:text-slate-100"
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
        className={`mt-3 ${button} bg-blue-600 hover:bg-blue-500`}
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
          className={`mt-2 text-sm ${message.ok ? "text-emerald-300" : "text-red-300"}`}
        >
          {message.text}
        </p>
      )}
    </section>
  );
}
