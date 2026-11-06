import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { ProtectionState } from "@/ipc";
import { mockProtection, resetMocks } from "@/ipc/testing";
import { ProtectionShield } from "./ProtectionShield";

describe("ProtectionShield", () => {
  beforeEach(resetMocks);

  it.each<[ProtectionState, string, string]>([
    ["protected", "Hidden from screen share", "bg-success"],
    ["not_guaranteed", "Screen-share hiding not guaranteed", "bg-warning"],
    ["unavailable", "Visible in screen share", "bg-danger"],
    ["disabled", "Screen-share hiding off", "bg-fg-muted"],
  ])("shows %s with label and colour", async (state, label, colour) => {
    mockProtection.status = { state, reason: "Because." };
    const { container } = render(<ProtectionShield />);
    const status = await screen.findByRole("status");
    expect(status).toHaveAccessibleName(`${label}. Because.`);
    expect(status).toHaveAttribute("title", "Because.");
    expect(container.querySelector(`.${colour}`)).not.toBeNull();
  });
});
