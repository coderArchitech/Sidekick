import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { choose } from "@/test/select";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCommands, mockFiles, mockSkills, resetMocks } from "@/ipc/testing";
import { SkillsSection } from "./SkillsSection";

async function renderSection() {
  const user = userEvent.setup();
  render(<SkillsSection />);
  const list = await screen.findByRole("list", { name: "Your Skills" });
  await waitFor(() => expect(within(list).getAllByRole("listitem")).toHaveLength(14));
  return { user, list };
}

const names = (list: HTMLElement) =>
  within(list)
    .getAllByRole("listitem")
    .map((li) => li.querySelector(".font-medium")?.textContent);

describe("Skills in Settings", () => {
  beforeEach(() => resetMocks());

  it("lists the built-in templates by name", async () => {
    const { list } = await renderSection();
    expect(names(list)).toEqual([
      "AI Engineering Interview (JS)",
      "Behavioural Round",
      "Design Discussion",
      "DS & Algo Round (JS)",
      "Interview Coach",
      "Managerial Round",
      "Marketing Meeting",
      "Negotiation Coach",
      "Sales Discovery",
      "Staff Engineer Interview (JS)",
      "System Design Round - Backend (JS)",
      "System Design Round - Frontend (JS)",
      "Teacher",
      "Technical Architect",
    ]);
  });

  it("builder creates a Skill from a template and saves it", async () => {
    const { user, list } = await renderSection();
    await choose(user, screen.getByRole("combobox", { name: "Start from a template" }), "teacher");
    const form = screen.getByRole("form", { name: "New Skill" });
    const name = within(form).getByLabelText("Name");
    expect(name).toHaveValue("Teacher (copy)");
    await user.clear(name);
    await user.type(name, "Physics Tutor");
    expect(within(form).getByText("Saved as physics-tutor.md")).toBeInTheDocument();
    expect((within(form).getByLabelText("Instructions") as HTMLTextAreaElement).value).toContain(
      "one-sentence explanation",
    );
    await user.click(within(form).getByRole("checkbox", { name: /Calculator/ }));
    await user.click(within(form).getByRole("button", { name: "Save Skill" }));

    expect(await screen.findByRole("status")).toHaveTextContent("Saved Physics Tutor.");
    const saved = mockSkills.get("physics-tutor");
    expect(saved?.tools).toEqual(["doc_search", "calculator"]);
    expect(saved?.builtin).toBe(false);
    await waitFor(() =>
      expect(names(screen.getByRole("list", { name: "Your Skills" }))).toContain("Physics Tutor"),
    );
    expect(list).toBeDefined();
  });

  it("shows field-level problems instead of saving", async () => {
    const { user } = await renderSection();
    await user.click(screen.getByRole("button", { name: "New Skill" }));
    const form = screen.getByRole("form", { name: "New Skill" });
    await user.click(within(form).getByRole("button", { name: "Save Skill" }));
    expect(
      within(form).getByText("Give the Skill a name (up to 80 characters)."),
    ).toBeInTheDocument();
    expect(
      within(form).getByText("Write the instructions below the front matter."),
    ).toBeInTheDocument();
    expect(mockSkills.list()).toHaveLength(14);
  });

  it("edits a Skill in place", async () => {
    const { user } = await renderSection();
    await user.click(screen.getByRole("button", { name: "Edit Teacher" }));
    const form = screen.getByRole("form", { name: "Edit Teacher" });
    const instr = within(form).getByLabelText("Instructions");
    await user.clear(instr);
    await user.type(instr, "Explain like I'm five.");
    await user.click(within(form).getByRole("button", { name: "Save Skill" }));
    await screen.findByText("Saved Teacher.");
    expect(mockSkills.get("teacher")?.instructions).toBe("Explain like I'm five.");
  });

  it("import lists the requested tools for approval and saves only when allowed", async () => {
    mockSkills.files.set(
      "/tmp/web.md",
      "---\nschema_version: 1\nid: web-helper\nname: Web Helper\ntools: [web_search, calculator]\n---\nSearch the web.\n",
    );
    const { user } = await renderSection();
    mockFiles.picked.push("/tmp/web.md");
    await user.click(screen.getByRole("button", { name: "Import…" }));
    const dialog = await screen.findByRole("dialog", { name: "Import Web Helper" });
    const tools = within(dialog).getByRole("list", { name: "Requested tools" });
    expect(tools).toHaveTextContent("Web search: Sends search queries");
    expect(tools).toHaveTextContent("Calculator");
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(mockSkills.get("web-helper")).toBeNull();

    mockFiles.picked.push("/tmp/web.md");
    await user.click(screen.getByRole("button", { name: "Import…" }));
    await user.click(await screen.findByRole("button", { name: "Allow and import" }));
    expect(await screen.findByText("Imported Web Helper.")).toBeInTheDocument();
    expect(mockSkills.get("web-helper")?.tools).toEqual(["web_search", "calculator"]);
  });

  it("import rejects unknown tools with the field named", async () => {
    mockSkills.files.set(
      "/tmp/evil.md",
      "---\nschema_version: 1\nid: evil\nname: Evil\ntools: [shell]\n---\nx\n",
    );
    const { user } = await renderSection();
    mockFiles.picked.push("/tmp/evil.md");
    await user.click(screen.getByRole("button", { name: "Import…" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      'tools[0]: "shell" isn\'t an allowed tool',
    );
  });

  it("export then import round trips", async () => {
    const { user } = await renderSection();
    mockFiles.saveAs.push("/tmp/out/teacher.md");
    await user.click(screen.getByRole("button", { name: "Export Teacher" }));
    await screen.findByText("Exported Teacher.");
    const r = await mockCommands.importSkill("/tmp/out/teacher.md");
    const teacher = mockSkills.get("teacher");
    expect(r.status === "ok" && r.data.skill).toEqual({
      id: teacher?.id,
      name: teacher?.name,
      description: teacher?.description,
      triggers: teacher?.triggers,
      tools: teacher?.tools,
      answerFormat: teacher?.answerFormat,
      knowledge: teacher?.knowledge,
      instructions: teacher?.instructions,
    });
    expect(r.status === "ok" && r.data.replacesExisting).toBe(true);
  });

  it("turns a Skill off and deletes one", async () => {
    const { user } = await renderSection();
    await user.click(screen.getByRole("checkbox", { name: "Teacher enabled" }));
    await waitFor(() => expect(mockSkills.get("teacher")?.enabled).toBe(false));
    await user.click(screen.getByRole("button", { name: "Delete Teacher" }));
    await waitFor(() => expect(mockSkills.get("teacher")).toBeNull());
  });
});
