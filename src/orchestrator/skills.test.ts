import { describe, expect, it } from "vitest";
import type { SkillEntry } from "@/ipc";
import { nextSkillId, skillFor } from "./skills";

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
