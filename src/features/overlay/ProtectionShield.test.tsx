import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { ProtectionState } from "@/ipc";
import { mockProtection, resetMocks } from "@/ipc/testing";
import { ProtectionShield } from "./ProtectionShield";

describe("ProtectionShield", () => {
  beforeEach(resetMocks);

  it.each<[ProtectionState, string, string]>([
    ["protected", "Hidden from screen share", "bg-emerald-500"],
    ["not_guaranteed", "Screen-share hiding not guaranteed", "bg-amber-400"],
    ["unavailable", "Visible in screen share", "bg-red-500"],
    ["disabled", "Screen-share hiding off", "bg-slate-400"],
  ])("shows %s with label and colour", async (state, label, colour) => {
    mockProtection.status = { state, reason: "Because." };
    const { container } = render(<ProtectionShield />);
    const status = await screen.findByRole("status");
    expect(status).toHaveAccessibleName(`${label}. Because.`);
    expect(status).toHaveAttribute("title", "Because.");
    expect(container.querySelector(`.${colour}`)).not.toBeNull();
  });
});
