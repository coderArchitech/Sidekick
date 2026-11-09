import { useId, useState } from "react";
import { Select } from "@/lib/Select";
import {
  commands,
  type AnswerFormat,
  type FieldError,
  type Skill,
  type Tool,
  type Trigger,
} from "@/ipc";
import { TOOL_LABELS, TRIGGER_LABELS } from "./labels";

const TOOLS = Object.keys(TOOL_LABELS) as Tool[];
const TRIGGERS = Object.keys(TRIGGER_LABELS) as Trigger[];

/** "Sales Discovery!" → "sales-discovery", like crates/skills `slug`. */
export function slug(name: string): string {
  const s = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64)
    .replace(/-+$/, "");
  return s || "skill";
}

interface Props {
  initial: Skill;
  /** True when editing a saved Skill: the id can't change. */
  editing: boolean;
  onSaved: (s: Skill) => void;
  onCancel: () => void;
}

/** The no-code Skill builder (PRD SKL-2). */
export function SkillBuilder({ initial, editing, onSaved, onCancel }: Props) {
  const base = useId();
  const [skill, setSkill] = useState<Skill>(initial);
  const [errors, setErrors] = useState<FieldError[]>([]);
  const [saving, setSaving] = useState(false);
  const errorFor = (field: string) =>
    errors
      .filter((e) => e.field === field || e.field.startsWith(`${field}[`))
      .map((e) => e.message);
  const set = (patch: Partial<Skill>) => setSkill((s) => ({ ...s, ...patch }));
  const toggle = <T,>(list: T[] | undefined, item: T) =>
    (list ?? []).includes(item) ? (list ?? []).filter((x) => x !== item) : [...(list ?? []), item];

  async function save() {
    setSaving(true);
    const toSave = { ...skill, id: editing ? skill.id : slug(skill.name) };
    const problems = await commands.checkSkill(toSave);
    if (problems.length) {
      setErrors(problems);
      setSaving(false);
      return;
    }
    const r = await commands.saveSkill(toSave);
    setSaving(false);
    if (r.status === "error") {
      setErrors([{ field: "", message: r.error.message }]);
      return;
    }
    onSaved(r.data);
  }

  const fieldError = (field: string) =>
    errorFor(field).map((m) => (
      <p key={m} className="text-xs text-danger">
        {m}
      </p>
    ));

  return (
    <form
      aria-label={editing ? `Edit ${initial.name}` : "New Skill"}
      className="space-y-3 rounded-md border border-line p-3 text-sm"
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <div>
        <label htmlFor={`${base}-name`} className="block text-fg-2">
          Name
        </label>
        <input
          id={`${base}-name`}
          value={skill.name}
          onChange={(e) => set({ name: e.target.value })}
          className="field mt-1 w-full"
        />
        {!editing && skill.name.trim() && (
          <p className="text-xs text-fg-muted">Saved as {slug(skill.name)}.md</p>
        )}
        {fieldError("name")}
        {fieldError("id")}
      </div>
      <div>
        <label htmlFor={`${base}-desc`} className="block text-fg-2">
          Description
        </label>
        <input
          id={`${base}-desc`}
          value={skill.description ?? ""}
          onChange={(e) => set({ description: e.target.value })}
          className="field mt-1 w-full"
        />
        {fieldError("description")}
      </div>
      <div>
        <label htmlFor={`${base}-instr`} className="block text-fg-2">
          Instructions
        </label>
        <p className="text-xs text-fg-muted">
          How Sidekick should behave: persona, tone, answer format, what to avoid.
        </p>
        <textarea
          id={`${base}-instr`}
          rows={8}
          value={skill.instructions}
          onChange={(e) => set({ instructions: e.target.value })}
          className="field mt-1 w-full"
        />
        {fieldError("instructions")}
      </div>
      <fieldset>
        <legend className="text-fg-2">Use for</legend>
        {TRIGGERS.map((t) => (
          <label key={t} className="mr-3 inline-flex items-center gap-1">
            <input
              type="checkbox"
              checked={(skill.triggers ?? TRIGGERS).includes(t)}
              onChange={() => set({ triggers: toggle(skill.triggers ?? TRIGGERS, t) })}
            />
            {TRIGGER_LABELS[t]}
          </label>
        ))}
        {fieldError("triggers")}
      </fieldset>
      <fieldset>
        <legend className="text-fg-2">Tools</legend>
        {TOOLS.map((t) => (
          <label key={t} className="block">
            <input
              type="checkbox"
              className="mr-1"
              checked={(skill.tools ?? []).includes(t)}
              onChange={() => set({ tools: toggle(skill.tools, t) })}
            />
            {TOOL_LABELS[t].label}
            <span className="text-xs text-fg-muted"> — {TOOL_LABELS[t].detail}</span>
          </label>
        ))}
        {fieldError("tools")}
      </fieldset>
      <div>
        <label htmlFor={`${base}-format`} className="block text-fg-2">
          Answers
        </label>
        <Select
          id={`${base}-format`}
          className="mt-1"
          value={skill.answerFormat ?? "cue_then_full"}
          onChange={(v) => set({ answerFormat: v as AnswerFormat })}
          options={[
            { value: "cue_then_full", label: "One-line cue, then the full answer" },
            { value: "full_only", label: "Full answer only" },
          ]}
        />
      </div>
      {errorFor("").map((m) => (
        <p key={m} role="alert" className="text-danger">
          {m}
        </p>
      ))}
      <div className="flex justify-end gap-2">
        <button type="button" className="btn btn-ghost" onClick={onCancel}>
          Cancel
        </button>
        <button type="submit" disabled={saving} className="btn btn-primary">
          Save Skill
        </button>
      </div>
    </form>
  );
}
