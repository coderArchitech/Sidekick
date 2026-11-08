import { describe, expect, it } from "vitest";
import type { SkillEntry } from "@/ipc";
import { nextSkillId, skillFor, skillsFor } from "./skills";

const skill = (id: string, over: Partial<SkillEntry> = {}): SkillEntry => ({
  id,
  name: id.toUpperCase(),
  instructions: `Be ${id}.`,
  triggers: ["question_detected", "chat", "screenshot"],
  tools: [],
  answerFormat: "cue_then_full",
  enabled: true,
  builtin: false,
  ...over,
});

describe("skillFor", () => {
  const skills = [
    skill("coach", { triggers: ["chat"] }),
    skill("off", { enabled: false }),
    skill("terse", { answerFormat: "full_only" }),
  ];

  it("applies the active Skill only where its triggers say", () => {
    expect(skillFor(skills, "coach", "chat")).toEqual({
      name: "COACH",
      instructions: "Be coach.",
      answerFormat: "cue_then_full",
    });
    expect(skillFor(skills, "coach", "detected")).toBeNull();
    expect(skillFor(skills, "coach", "screenshot")).toBeNull();
  });

  it("ignores no Skill, unknown and disabled Skills", () => {
    expect(skillFor(skills, null, "chat")).toBeNull();
    expect(skillFor(skills, "missing", "chat")).toBeNull();
    expect(skillFor(skills, "off", "chat")).toBeNull();
  });

  it("carries the answer format", () => {
    expect(skillFor(skills, "terse", "detected")?.answerFormat).toBe("full_only");
  });
});

describe("nextSkillId (Switch Skill)", () => {
  const skills = [skill("a"), skill("b", { enabled: false }), skill("c")];
  it("cycles enabled Skills, then none", () => {
    expect(nextSkillId(skills, null)).toBe("a");
    expect(nextSkillId(skills, "a")).toBe("c");
    expect(nextSkillId(skills, "c")).toBeNull();
    expect(nextSkillId(skills, "b")).toBe("a");
    expect(nextSkillId([], null)).toBeNull();
  });
});

describe("skillsFor: several active Skills (PRD SKL-5)", () => {
  const skills = [
    skill("coach", { triggers: ["chat"], tools: ["calculator"] }),
    skill("architect", { tools: ["doc_search", "calculator"] }),
    skill("terse", { answerFormat: "full_only" }),
    skill("brief", { answerFormat: "full_only" }),
    skill("off", { enabled: false }),
  ];

  it("combines the Skills that apply, main first, pooling tools", () => {
    expect(skillsFor(skills, ["coach", "architect", "off"], "chat")).toEqual({
      name: "COACH + ARCHITECT",
      instructions: "### COACH\nBe coach.\n\n### ARCHITECT\nBe architect.",
      answerFormat: "cue_then_full",
      tools: ["calculator", "doc_search"],
    });
    // Coach only applies to Chat.
    expect(skillsFor(skills, ["coach", "architect"], "detected")?.name).toBe("ARCHITECT");
  });

  it("keeps the cue unless every Skill is full-only", () => {
    expect(skillsFor(skills, ["terse", "architect"], "chat")?.answerFormat).toBe("cue_then_full");
    expect(skillsFor(skills, ["terse", "brief"], "chat")?.answerFormat).toBe("full_only");
  });

  it("is a single Skill's prompt, or null, otherwise", () => {
    expect(skillsFor(skills, ["architect", "architect", null], "chat")).toEqual(
      skillFor(skills, "architect", "chat"),
    );
    expect(skillsFor(skills, [null, "off"], "chat")).toBeNull();
  });
});
