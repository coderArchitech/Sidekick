import { useEffect, useId, useState } from "react";
import { Select } from "@/lib/Select";
import {
  commands,
  type ModelInfo,
  type ProviderChoice,
  type ProviderEntry,
  type ProviderKind,
} from "@/ipc";
import { KeyField } from "./KeyField";
import { WhisperModels } from "./WhisperModels";

interface Props {
  kind: ProviderKind;
  title: string;
  description: string;
  choice: ProviderChoice | null;
  /** Adds an "Off" option (web search is optional). */
  allowOff?: boolean;
  onChoose: (choice: ProviderChoice | null) => void;
}

const OFF = "__off__";
const DEFAULT_MODEL = "";

/** One provider kind: provider dropdown, its key, and (for LLMs) a model dropdown (PRD KEY-1 to KEY-3a, KEY-6). */
export function ProviderSection({ kind, title, description, choice, allowOff, onChoose }: Props) {
  const providerSelectId = useId();
  const modelSelectId = useId();
  const [providers, setProviders] = useState<ProviderEntry[]>([]);
  // Bumped after a key is saved or removed, to reload key status.
  const [version, setVersion] = useState(0);
  const [loadedModels, setLoadedModels] = useState<{ provider: string; models: ModelInfo[] }>({
    provider: "",
    models: [],
  });

  useEffect(() => {
    let cancelled = false;
    void commands.listProviders(kind).then((result) => {
      if (!cancelled && result.status === "ok") setProviders(result.data);
    });
    return () => {
      cancelled = true;
    };
  }, [kind, version]);

  const selected = providers.find((p) => p.id === choice?.provider);
  const canListModels =
    kind === "llm" &&
    selected !== undefined &&
    selected.implemented &&
    (!selected.keyRequired || selected.key.saved);
  const selectedId = selected?.id;

  useEffect(() => {
    if (!canListModels || !selectedId) return;
    let cancelled = false;
    void commands.listModels(selectedId).then((result) => {
      if (!cancelled) {
        setLoadedModels({
          provider: selectedId,
          models: result.status === "ok" ? result.data : [],
        });
      }
    });
    return () => {
      cancelled = true;
    };
  }, [canListModels, selectedId, version]);

  const models = canListModels && loadedModels.provider === selectedId ? loadedModels.models : [];

  return (
    <section aria-labelledby={`${providerSelectId}-heading`} className="card p-4">
      <h2 id={`${providerSelectId}-heading`} className="heading text-xl">
        {title}
      </h2>
      <p className="mb-3 text-sm text-fg-muted">{description}</p>

      <label htmlFor={providerSelectId} className="block text-sm text-fg-2">
        Provider
      </label>
      <Select
        id={providerSelectId}
        className="mt-1 max-w-sm"
        value={choice?.provider ?? OFF}
        onChange={(v) => onChoose(v === OFF ? null : { provider: v, model: null })}
        options={[
          ...(allowOff ? [{ value: OFF, label: "Off" }] : []),
          ...providers.map((p) => ({
            value: p.id,
            label: `${p.displayName}${p.key.saved ? " (key saved)" : ""}${p.implemented ? "" : " (coming soon)"}`,
          })),
        ]}
      />

      {selected && !selected.keyRequired && (
        <p className="mt-3 text-sm text-fg-muted">Runs on this computer. No key needed.</p>
      )}
      {selected?.keyRequired && (
        <KeyField
          key={selected.id}
          provider={selected.id}
          providerName={selected.displayName}
          status={selected.key}
          onChanged={() => setVersion((v) => v + 1)}
        />
      )}

      {kind === "llm" && models.length > 0 && choice && (
        <div className="mt-3">
          <label htmlFor={modelSelectId} className="block text-sm text-fg-2">
            Model
          </label>
          <Select
            id={modelSelectId}
            className="mt-1 max-w-sm"
            value={choice.model ?? DEFAULT_MODEL}
            onChange={(v) => onChoose({ provider: choice.provider, model: v || null })}
            options={[
              { value: DEFAULT_MODEL, label: "Sidekick default" },
              ...models.map((m) => ({ value: m.id, label: m.displayName })),
            ]}
          />
        </div>
      )}
      {selected?.id === "whisper-local" && choice && (
        <WhisperModels
          model={choice.model}
          onChoose={(model) => onChoose({ provider: choice.provider, model })}
        />
      )}
      {kind !== "llm" && selected?.id !== "whisper-local" && choice?.model && (
        <p className="mt-3 text-sm text-fg-muted">Model: {choice.model}</p>
      )}
    </section>
  );
}
