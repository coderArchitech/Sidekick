import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { choose } from "@/test/select";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockCommands, resetMocks } from "@/ipc/testing";
import { SettingsWindow } from "./SettingsWindow";

describe("Local Whisper in Settings", () => {
  beforeEach(() => resetMocks());

  async function chooseWhisper() {
    const user = userEvent.setup();
    render(<SettingsWindow />);
    const stt = await screen.findByRole("region", { name: /speech-to-text/i });
    const provider = within(stt).getByRole("combobox", { name: "Provider" });
    await waitFor(() => expect(provider).toHaveTextContent("Deepgram"));
    await choose(user, within(stt).getByRole("combobox", { name: "Provider" }), "whisper-local");
    return { user, stt };
  }

  it("needs no key and offers the multilingual base model by default", async () => {
    const { stt } = await chooseWhisper();
    expect(within(stt).getByText("Runs on this computer. No key needed.")).toBeInTheDocument();
    const model = await within(stt).findByRole("combobox", { name: "Model" });
    await waitFor(() => expect(model).toHaveAttribute("data-value", "base"));
    expect(within(stt).getByText(/Hinglish/)).toBeInTheDocument();
    expect(within(stt).getByRole("button", { name: "Download (142 MB)" })).toBeInTheDocument();
  });

  it("downloads the chosen model with progress, then shows it ready", async () => {
    const download = vi.spyOn(mockCommands, "downloadWhisperModel");
    const { user, stt } = await chooseWhisper();
    const model = await within(stt).findByRole("combobox", { name: "Model" });
    await choose(user, model, "tiny.en");
    await waitFor(() => expect(model).toHaveAttribute("data-value", "tiny.en"));
    await user.click(within(stt).getByRole("button", { name: "Download (75 MB)" }));
    expect(download).toHaveBeenCalledWith("tiny.en");
    expect(
      await within(stt).findByText("Ready. Audio stays on this computer."),
    ).toBeInTheDocument();
    expect(within(stt).getByRole("combobox", { name: "Model" })).toHaveTextContent(
      /tiny\.en.*\(downloaded\)/,
    );
  });

  it("shows a failed checksum", async () => {
    vi.spyOn(mockCommands, "downloadWhisperModel").mockResolvedValue({
      status: "error",
      error: {
        code: "PARSE_FAILED",
        message: "The downloaded model didn't match its checksum, so it was deleted. Try again.",
        retryable: false,
      },
    });
    const { user, stt } = await chooseWhisper();
    await user.click(await within(stt).findByRole("button", { name: /Download/ }));
    expect(await within(stt).findByRole("alert")).toHaveTextContent("checksum");
  });
});
