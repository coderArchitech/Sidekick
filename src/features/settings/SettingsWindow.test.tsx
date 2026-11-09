import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockCommands, mockWindowCalls, resetMocks } from "@/ipc/testing";
import { SettingsWindow } from "./SettingsWindow";

function section(name: RegExp) {
  return screen.getByRole("region", { name });
}

async function renderSettings() {
  render(<SettingsWindow />);
  await screen.findByRole("region", { name: /speech-to-text/i });
  // Wait for every provider list to load.
  await waitFor(() => expect(screen.getAllByRole("option", { name: /deepgram/i })).toHaveLength(1));
}

describe("SettingsWindow", () => {
  beforeEach(() => {
    resetMocks();
  });

  it("shows a section per provider kind with the defaults selected", async () => {
    await renderSettings();
    const provider = (re: RegExp) =>
      within(section(re)).getByRole("combobox", { name: "Provider" }) as HTMLSelectElement;
    expect(provider(/speech-to-text/i).value).toBe("deepgram");
    expect(provider(/answers/i).value).toBe("anthropic");
    expect(provider(/embeddings/i).value).toBe("openai");
    expect(provider(/web search/i).value).toBe("__off__");
    expect(
      within(section(/answers/i))
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["Anthropic (Claude)", "OpenAI", "Google Gemini", "Ollama (local)"]);
  });

  it("picks a theme for both windows (UI-8)", async () => {
    const user = userEvent.setup();
    await renderSettings();
    const theme = screen.getByRole("combobox", { name: "Theme" });
    expect(theme).toHaveValue("dark");
    expect(
      within(theme)
        .getAllByRole("option")
        .map((o) => o.textContent),
    ).toEqual(["Dark", "Light", "High contrast", "Same as the system"]);
    await user.selectOptions(theme, "high_contrast");
    await waitFor(async () => {
      const s = await mockCommands.getSettings();
      expect(s.status === "ok" && s.data.theme).toBe("high_contrast");
    });
  });

  it("lists the Click-through shortcut (UI-5)", async () => {
    await renderSettings();
    expect(screen.getByText("Click-through on/off")).toBeInTheDocument();
  });

  it("masks the key by default and toggles visibility", async () => {
    const user = userEvent.setup();
    await renderSettings();
    const llm = section(/answers/i);
    const input = within(llm).getByLabelText("Anthropic (Claude) API key");
    expect(input).toHaveAttribute("type", "password");
    await user.click(within(llm).getByRole("button", { name: "Show" }));
    expect(input).toHaveAttribute("type", "text");
    expect(within(llm).getByRole("button", { name: "Hide" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("pastes from the clipboard", async () => {
    const user = userEvent.setup();
    vi.spyOn(navigator.clipboard, "readText").mockResolvedValue("  sk-ant-pasted9999 ");
    await renderSettings();
    const llm = section(/answers/i);
    await user.click(within(llm).getByRole("button", { name: "Paste" }));
    expect(within(llm).getByLabelText("Anthropic (Claude) API key")).toHaveValue(
      "sk-ant-pasted9999",
    );
  });

  it("saves a working key, clears the input and shows only the last 4", async () => {
    const user = userEvent.setup();
    await renderSettings();
    const llm = section(/answers/i);
    await user.type(within(llm).getByLabelText("Anthropic (Claude) API key"), "sk-ant-secret-1234");
    await user.click(within(llm).getByRole("button", { name: "Save and test" }));
    expect(await within(llm).findByRole("status")).toHaveTextContent("Key works and is saved.");
    expect(within(llm).getByText("••••1234")).toBeInTheDocument();
    expect(within(llm).queryByLabelText("Anthropic (Claude) API key")).not.toBeInTheDocument();
    expect(llm.textContent).not.toContain("secret");
  });

  it("explains a rejected key and does not save it", async () => {
    const user = userEvent.setup();
    await renderSettings();
    const llm = section(/answers/i);
    await user.type(within(llm).getByLabelText("Anthropic (Claude) API key"), "sk-invalid-0000");
    await user.click(within(llm).getByRole("button", { name: "Save and test" }));
    expect(await within(llm).findByRole("alert")).toHaveTextContent(/rejected this key/);
    const providers = await mockCommands.listProviders("llm");
    expect(providers.status === "ok" && providers.data[0]?.key.saved).toBe(false);
  });

  it("tests and removes a saved key", async () => {
    const user = userEvent.setup();
    await mockCommands.setApiKey("anthropic", "sk-ant-saved-5678");
    await renderSettings();
    const llm = section(/answers/i);
    await user.click(await within(llm).findByRole("button", { name: "Test connection" }));
    expect(await within(llm).findByRole("status")).toHaveTextContent("Connection works.");
    await user.click(within(llm).getByRole("button", { name: "Remove key" }));
    expect(await within(llm).findByLabelText("Anthropic (Claude) API key")).toBeInTheDocument();
  });

  it("lists models once a key is saved and persists the choice", async () => {
    const user = userEvent.setup();
    await mockCommands.setApiKey("anthropic", "sk-ant-saved-5678");
    await renderSettings();
    const llm = section(/answers/i);
    const model = await within(llm).findByRole("combobox", { name: "Model" });
    await user.selectOptions(model, "mock-large");
    await waitFor(async () => {
      const s = await mockCommands.getSettings();
      expect(s.status === "ok" && s.data.llm).toEqual({
        provider: "anthropic",
        model: "mock-large",
      });
    });
  });

  it("switches providers without re-entering saved keys", async () => {
    const user = userEvent.setup();
    await mockCommands.setApiKey("anthropic", "sk-ant-saved-5678");
    await renderSettings();
    const llm = section(/answers/i);
    const provider = within(llm).getByRole("combobox", { name: "Provider" });
    await user.selectOptions(provider, "ollama");
    expect(await within(llm).findByText(/no key needed/i)).toBeInTheDocument();
    await user.selectOptions(provider, "anthropic");
    expect(await within(llm).findByText("••••5678")).toBeInTheDocument();
    const s = await mockCommands.getSettings();
    expect(s.status === "ok" && s.data.llm.provider).toBe("anthropic");
  });

  it("turns web search on and off", async () => {
    const user = userEvent.setup();
    await renderSettings();
    const search = within(section(/web search/i)).getByRole("combobox", { name: "Provider" });
    await user.selectOptions(search, "tavily");
    await waitFor(async () => {
      const s = await mockCommands.getSettings();
      expect(s.status === "ok" && s.data.search?.provider).toBe("tavily");
    });
    await user.selectOptions(search, "__off__");
    await waitFor(async () => {
      const s = await mockCommands.getSettings();
      expect(s.status === "ok" && s.data.search).toBeNull();
    });
  });

  it("saves general settings", async () => {
    const user = userEvent.setup();
    await renderSettings();
    await user.click(screen.getByRole("checkbox", { name: /auto-answer/i }));
    await user.selectOptions(screen.getByRole("combobox", { name: "Keep session data" }), "7d");
    await waitFor(async () => {
      const s = await mockCommands.getSettings();
      expect(s.status === "ok" && [s.data.autoAnswer, s.data.retention]).toEqual([false, "7d"]);
    });
  });

  it("closes by hiding the window", async () => {
    const user = userEvent.setup();
    await renderSettings();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(mockWindowCalls.hide).toBe(1);
  });

  it("has no accessibility violations", async () => {
    const { container } = render(<SettingsWindow />);
    await screen.findByRole("region", { name: /speech-to-text/i });
    const results = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
    expect(results.violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
  });
});
