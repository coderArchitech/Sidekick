import { useEffect, useId, useState } from "react";
import { Select } from "@/lib/Select";
import {
  chooseSkillExportPath,
  commands,
  pickSkillFile,
  type ImportPreview,
  type Skill,
  type SkillEntry,
} from "@/ipc";
import { SkillBuilder } from "./SkillBuilder";
import { TOOL_LABELS } from "./labels";

const BLANK: Skill = {
  id: "",
  name: "",
  description: "",
  triggers: ["question_detected", "chat", "screenshot"],
  tools: [],
  answerFormat: "cue_then_full",
  knowledge: [],
  instructions: "",
};

type Editing = { skill: Skill; existing: boolean } | null;

/** Settings → Skills (PRD SKL-1 to SKL-3): list, builder, templates, import, export. */
export function SkillsSection() {
  const headingId = useId();
  const [skills, setSkills] = useState<SkillEntry[]>([]);
  const [templates, setTemplates] = useState<Skill[]>([]);
  const [editing, setEditing] = useState<Editing>(null);
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);
  const [version, setVersion] = useState(0);

  useEffect(() => {
    void commands.listSkills().then((r) => r.status === "ok" && setSkills(r.data));
  }, [version]);
  useEffect(() => {
    void commands.skillTemplates().then((r) => r.status === "ok" && setTemplates(r.data));
  }, []);

  const reload = () => setVersion((v) => v + 1);
  const fail = (text: string) => setMessage({ text, error: true });

  async function startImport() {
    setMessage(null);
    const path = await pickSkillFile();
    if (!path) return;
    const r = await commands.importSkill(path);
    if (r.status === "error") fail(r.error.message);
    else setPreview(r.data);
  }

  async function approveImport() {
    if (!preview) return;
    const r = await commands.saveSkill(preview.skill);
    setPreview(null);
    if (r.status === "error") return fail(r.error.message);
    setMessage({ text: `Imported ${r.data.name}.`, error: false });
    reload();
  }

  async function exportSkill(s: SkillEntry) {
    setMessage(null);
    const path = await chooseSkillExportPath(`${s.id}.md`);
    if (!path) return;
    const r = await commands.exportSkill(s.id, path);
    if (r.status === "error") fail(r.error.message);
    else setMessage({ text: `Exported ${s.name}.`, error: false });
  }

  async function remove(s: SkillEntry) {
    const r = await commands.deleteSkill(s.id);
    if (r.status === "error") fail(r.error.message);
    reload();
  }

  async function setEnabled(s: SkillEntry, enabled: boolean) {
    const r = await commands.setSkillEnabled(s.id, enabled);
    if (r.status === "error") fail(r.error.message);
    reload();
  }

  return (
    <section aria-labelledby={headingId} className="card p-4">
      <h2 id={headingId} className="heading text-xl">
        Skills
      </h2>
      <p className="mb-3 text-sm text-fg-muted">
        Tell Sidekick how to behave for a kind of meeting. Pick one for a session from the overlay.
      </p>

      {message && (
        <p
          role={message.error ? "alert" : "status"}
          className={`mb-2 text-sm ${message.error ? "text-danger" : "text-success"}`}
        >
          {message.text}
        </p>
      )}

      {editing ? (
        <SkillBuilder
          key={editing.skill.id || "new"}
          initial={editing.skill}
          editing={editing.existing}
          onCancel={() => setEditing(null)}
          onSaved={(s) => {
            setEditing(null);
            setMessage({ text: `Saved ${s.name}.`, error: false });
            reload();
          }}
        />
      ) : preview ? (
        <div
          role="dialog"
          aria-label={`Import ${preview.skill.name}`}
          className="space-y-2 rounded-md border border-warning p-3 text-sm"
        >
          <p className="font-medium">Import “{preview.skill.name}”?</p>
          {preview.skill.description && (
            <p className="text-fg-muted">{preview.skill.description}</p>
          )}
          {preview.requestedTools.length ? (
            <>
              <p>This Skill asks to use:</p>
              <ul aria-label="Requested tools" className="ml-4 list-disc">
                {preview.requestedTools.map((t) => (
                  <li key={t}>
                    {TOOL_LABELS[t].label}: {TOOL_LABELS[t].detail}
                  </li>
                ))}
              </ul>
            </>
          ) : (
            <p>This Skill uses no tools.</p>
          )}
          {preview.replacesExisting && (
            <p className="text-warning">
              It replaces your Skill with the same id ({preview.skill.id}).
            </p>
          )}
          <div className="flex justify-end gap-2">
            <button type="button" className="btn btn-ghost" onClick={() => setPreview(null)}>
              Cancel
            </button>
            <button type="button" className="btn btn-primary" onClick={() => void approveImport()}>
              Allow and import
            </button>
          </div>
        </div>
      ) : (
        <>
          <ul aria-label="Your Skills" className="divide-y divide-line text-sm">
            {skills.map((s) => (
              <li key={s.id} className="flex items-center gap-2 py-1.5">
                <span className="min-w-0 flex-1">
                  <span className="font-medium">{s.name}</span>
                  {s.description && (
                    <span className="block truncate text-xs text-fg-muted">{s.description}</span>
                  )}
                </span>
                <label className="flex items-center gap-1 text-xs text-fg-muted">
                  <input
                    type="checkbox"
                    checked={s.enabled}
                    aria-label={`${s.name} enabled`}
                    onChange={(e) => void setEnabled(s, e.target.checked)}
                  />
                  On
                </label>
                <button
                  type="button"
                  className="rounded px-1.5 hover:bg-surface-2"
                  aria-label={`Edit ${s.name}`}
                  onClick={() => setEditing({ skill: s, existing: true })}
                >
                  Edit
                </button>
                <button
                  type="button"
                  className="rounded px-1.5 hover:bg-surface-2"
                  aria-label={`Export ${s.name}`}
                  onClick={() => void exportSkill(s)}
                >
                  Export
                </button>
                <button
                  type="button"
                  className="rounded px-1.5 text-danger hover:bg-surface-2"
                  aria-label={`Delete ${s.name}`}
                  onClick={() => void remove(s)}
                >
                  Delete
                </button>
              </li>
            ))}
          </ul>
          <div className="mt-3 flex flex-wrap items-center gap-2 text-sm">
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => setEditing({ skill: BLANK, existing: false })}
            >
              New Skill
            </button>
            <div className="flex items-center gap-1 text-fg-2">
              From template
              <Select
                aria-label="Start from a template"
                className="w-56"
                value=""
                onChange={(v) => {
                  const t = templates.find((x) => x.id === v);
                  if (t)
                    setEditing({
                      skill: { ...t, id: "", name: `${t.name} (copy)` },
                      existing: false,
                    });
                }}
                options={[
                  { value: "", label: "Choose…" },
                  ...templates.map((t) => ({ value: t.id, label: t.name })),
                ]}
              />
            </div>
            <button type="button" className="btn btn-secondary" onClick={() => void startImport()}>
              Import…
            </button>
          </div>
        </>
      )}
    </section>
  );
}
