import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCommands, resetMocks } from "@/ipc/testing";
import { VocabularySection } from "./VocabularySection";

const save = async (vocabulary: string[]) => {
  const r = await mockCommands.updateSettings({ vocabulary });
  return r.status === "ok" ? { saved: r.data.vocabulary } : { error: r.error.message };
};

describe("Settings → Custom vocabulary (PRD LIS-5)", () => {
  beforeEach(resetMocks);

  it("saves one term per line or comma, cleaned, and counts them", async () => {
    const user = userEvent.setup();
    render(<VocabularySection vocabulary={[]} onSave={save} />);
    const box = screen.getByLabelText("Vocabulary terms");
    await user.type(box, "Kubernetes, SOC 2{Enter}  kubernetes {Enter}Priya   Raman");
    expect(screen.getByText("4/100 terms")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save vocabulary" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Saved 3 terms. They apply from the next session.",
    );
    expect(box).toHaveValue("Kubernetes\nSOC 2\nPriya Raman");
    const s = await mockCommands.getSettings();
    expect(s.status === "ok" && s.data.vocabulary).toEqual(["Kubernetes", "SOC 2", "Priya Raman"]);
  });

  it("shows the limit error", async () => {
    const user = userEvent.setup();
    render(<VocabularySection vocabulary={[]} onSave={save} />);
    await user.type(screen.getByLabelText("Vocabulary terms"), "x".repeat(51));
    await user.click(screen.getByRole("button", { name: "Save vocabulary" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("up to 100 terms");
  });

  it("clears, and has no accessibility violations", async () => {
    const user = userEvent.setup();
    const { container } = render(<VocabularySection vocabulary={["SOC 2"]} onSave={save} />);
    await user.clear(screen.getByLabelText("Vocabulary terms"));
    await user.click(screen.getByRole("button", { name: "Save vocabulary" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Vocabulary cleared.");
    const results = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
    expect(results.violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
  });
});
