/**
 * Browser mocks for Skills, mirroring crates/skills: the same templates,
 * the same validation rules (schemas/skill.schema.json) and import preview.
 */
import interviewCoach from "../../skills/templates/interview-coach.md?raw";
import negotiationCoach from "../../skills/templates/negotiation-coach.md?raw";
import salesDiscovery from "../../skills/templates/sales-discovery.md?raw";
import teacher from "../../skills/templates/teacher.md?raw";
import technicalArchitect from "../../skills/templates/technical-architect.md?raw";
import type {
  AnswerFormat,
  FieldError,
  ImportPreview,
  Skill,
  SkillEntry,
  Tool,
  Trigger,
} from "./bindings";

const TEMPLATE_TEXT = [
  interviewCoach,
  salesDiscovery,
  technicalArchitect,
  teacher,
  negotiationCoach,
];
export const TOOLS: Tool[] = ["doc_search", "web_search", "calculator"];
export const TRIGGERS: Trigger[] = ["question_detected", "chat", "screenshot"];
const ID = /^[a-z0-9][a-z0-9-]{0,63}$/;
const KEYS = [
  "schema_version",
  "id",
  "name",
  "description",
  "triggers",
  "tools",
  "answer_format",
  "knowledge",
];

/** Parses the simple YAML the templates and exports use (scalars and [a, b] lists). */
function parseFront(text: string): { front: Record<string, unknown>; body: string } | null {
  const m = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/.exec(text.replace(/^\uFEFF/, ""));
  if (!m) return null;
  const front: Record<string, unknown> = {};
  for (const line of (m[1] ?? "").split(/\r?\n/)) {
    const kv = /^([a-z_]+):\s*(.*)$/.exec(line);
    if (!kv) continue;
    const [, key = "", raw = ""] = kv;
    const v = raw.trim();
    if (v.startsWith("[") && v.endsWith("]")) {
      front[key] = v
        .slice(1, -1)
        .split(",")
        .map((s) => s.trim().replace(/^['"]|['"]$/g, ""))
        .filter(Boolean);
    } else if (/^\d+$/.test(v)) {
      front[key] = Number(v);
    } else {
      front[key] = v.replace(/^['"]|['"]$/g, "");
    }
  }
  return { front, body: (m[2] ?? "").trim() };
}

export function validateSkill(s: Skill): FieldError[] {
  const errors: FieldError[] = [];
  if (!ID.test(s.id))
    errors.push({ field: "id", message: "Use lowercase letters, digits and dashes (up to 64)." });
  if (!s.name.trim() || s.name.length > 80)
    errors.push({ field: "name", message: "Give the Skill a name (up to 80 characters)." });
  if ((s.description ?? "").length > 300)
    errors.push({ field: "description", message: "Keep the description under 300 characters." });
  (s.tools ?? []).forEach((t, i) => {
    if (!TOOLS.includes(t))
      errors.push({
        field: `tools[${i}]`,
        message: `"${String(t)}" isn't an allowed tool. Allowed: doc_search, web_search, calculator.`,
      });
  });
  (s.triggers ?? []).forEach((t, i) => {
    if (!TRIGGERS.includes(t))
      errors.push({
        field: `triggers[${i}]`,
        message: `"${String(t)}" isn't a trigger. Use question_detected, chat or screenshot.`,
      });
  });
  if (!s.instructions.trim())
    errors.push({
      field: "instructions",
      message: "Write the instructions below the front matter.",
    });
  else if (s.instructions.trim().length > 8000)
    errors.push({ field: "instructions", message: "Keep the instructions under 8000 characters." });
  return errors;
}

export function parseSkill(text: string): { skill: Skill } | { errors: FieldError[] } {
  const p = parseFront(text);
  if (!p)
    return {
      errors: [{ field: "", message: "A Skill file starts with front matter between --- lines." }],
    };
  const f = p.front;
  const unknown = Object.keys(f).filter((k) => !KEYS.includes(k));
  const skill: Skill = {
    id: String(f.id ?? ""),
    name: String(f.name ?? ""),
    description: String(f.description ?? ""),
    triggers: (f.triggers as Trigger[] | undefined) ?? [...TRIGGERS],
    tools: (f.tools as Tool[] | undefined) ?? [],
    answerFormat: (f.answer_format as AnswerFormat | undefined) ?? "cue_then_full",
    knowledge: (f.knowledge as string[] | undefined) ?? [],
    instructions: p.body,
  };
  const errors = validateSkill(skill);
  if (f.schema_version !== 1) errors.unshift({ field: "schema_version", message: "Must be 1." });
  for (const k of unknown) errors.push({ field: "", message: `Unknown field "${k}".` });
  return errors.length ? { errors } : { skill };
}

export function toMarkdown(s: Skill): string {
  const list = (xs: string[] | undefined) => `[${(xs ?? []).join(", ")}]`;
  const lines = [
    "---",
    "schema_version: 1",
    `id: ${s.id}`,
    `name: ${s.name}`,
    ...(s.description ? [`description: ${s.description}`] : []),
    `triggers: ${list(s.triggers)}`,
    `tools: ${list(s.tools)}`,
    `answer_format: ${s.answerFormat ?? "cue_then_full"}`,
    ...(s.knowledge?.length ? [`knowledge: ${list(s.knowledge)}`] : []),
    "---",
    s.instructions.trim(),
    "",
  ];
  return lines.join("\n");
}

export const TEMPLATES: Skill[] = TEMPLATE_TEXT.map((t) => {
  const r = parseSkill(t);
  if (!("skill" in r)) throw new Error(`bad template: ${JSON.stringify(r.errors)}`);
  return r.skill;
});

/** In-memory Skills store with the same rules as crates/skills. */
export function createSkillStore() {
  const skills = new Map<string, SkillEntry>();
  /** Files the mocked dialogs read and write: path → contents. */
  const files = new Map<string, string>();
  const builtin = new Set(TEMPLATES.map((t) => t.id));
  const seed = () => {
    skills.clear();
    for (const t of TEMPLATES)
      skills.set(t.id, { ...structuredClone(t), enabled: true, builtin: true });
  };
  seed();
  return {
    files,
    reset: () => {
      seed();
      files.clear();
    },
    list: (): SkillEntry[] =>
      [...skills.values()]
        .sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }))
        .map((s) => structuredClone(s)),
    has: (id: string) => skills.has(id),
    save: (s: Skill): SkillEntry | FieldError[] => {
      const errors = validateSkill(s);
      if (errors.length) return errors;
      const entry: SkillEntry = {
        ...structuredClone(s),
        name: s.name.trim(),
        instructions: s.instructions.trim(),
        enabled: skills.get(s.id)?.enabled ?? true,
        builtin: builtin.has(s.id),
      };
      skills.set(s.id, entry);
      return structuredClone(entry);
    },
    get: (id: string) => {
      const s = skills.get(id);
      return s ? structuredClone(s) : null;
    },
    setEnabled: (id: string, enabled: boolean) => {
      const s = skills.get(id);
      if (!s) return null;
      s.enabled = enabled;
      return structuredClone(s);
    },
    delete: (id: string) => skills.delete(id),
    preview: (path: string): ImportPreview | FieldError[] | null => {
      const text = files.get(path);
      if (text === undefined) return null;
      const r = parseSkill(text);
      if ("errors" in r) return r.errors;
      return {
        skill: r.skill,
        requestedTools: r.skill.tools ?? [],
        replacesExisting: skills.has(r.skill.id),
      };
    },
  };
}
