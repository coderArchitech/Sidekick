import { useEffect, useId, useState } from "react";
import { Select } from "@/lib/Select";
import { commands, sessionEvents, type WhisperModelEntry } from "@/ipc";

const DEFAULT = "base";

interface Props {
  model: string | null;
  onChoose: (model: string) => void;
}

/** Local Whisper: pick a model and download it, checked against its checksum (PRD KEY-2). */
export function WhisperModels({ model, onChoose }: Props) {
  const selectId = useId();
  const [models, setModels] = useState<WhisperModelEntry[]>([]);
  const [progress, setProgress] = useState<{ name: string; percent: number | null } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [version, setVersion] = useState(0);
  const chosen = model ?? DEFAULT;
  const entry = models.find((m) => m.name === chosen);

  useEffect(() => {
    void commands.whisperModels().then(setModels);
  }, [version]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void sessionEvents
      .onModelDownloadProgress((e) =>
        setProgress({
          name: e.name,
          percent: e.totalBytes ? Math.floor((e.doneBytes / e.totalBytes) * 100) : null,
        }),
      )
      .then((u) => (disposed ? u() : (unlisten = u)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function download() {
    setError(null);
    setProgress({ name: chosen, percent: 0 });
    const r = await commands.downloadWhisperModel(chosen);
    setProgress(null);
    if (r.status === "error") setError(r.error.message);
    setVersion((v) => v + 1);
  }

  const busy = progress !== null;
  return (
    <div className="mt-3 space-y-2 text-sm">
      <label htmlFor={selectId} className="block text-fg-2">
        Model
      </label>
      <Select
        id={selectId}
        className="max-w-md"
        value={chosen}
        disabled={busy}
        onChange={onChoose}
        options={models.map((m) => ({
          value: m.name,
          label: `${m.name} · ${m.sizeMb} MB · ${m.englishOnly ? "English" : "multilingual"}${m.downloaded ? " (downloaded)" : ""}`,
        }))}
      />
      {entry && !entry.downloaded && !busy && (
        <button type="button" className="btn btn-primary ml-2" onClick={() => void download()}>
          Download ({entry.sizeMb} MB)
        </button>
      )}
      {entry?.downloaded && <p className="text-success">Ready. Audio stays on this computer.</p>}
      {progress && (
        <div role="status" aria-label="Download progress">
          <progress
            className="w-full"
            max={100}
            value={progress.percent ?? undefined}
            aria-valuetext={progress.percent === null ? "Downloading" : `${progress.percent}%`}
          />
          <p className="text-fg-muted">
            Downloading {progress.name}… {progress.percent === null ? "" : `${progress.percent}%`}
          </p>
        </div>
      )}
      {error && (
        <p role="alert" className="text-danger">
          {error}
        </p>
      )}
      {entry && !entry.englishOnly && (
        <p className="text-fg-muted">
          Multilingual models also handle mixed languages like Hinglish.
        </p>
      )}
    </div>
  );
}
