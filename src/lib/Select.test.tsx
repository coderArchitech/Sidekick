import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { choose } from "@/test/select";
import { Select } from "./Select";

const OPTIONS = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
  { value: "hc", label: "High contrast", disabled: true },
  { value: "system", label: "System" },
];

function Harness({ onChange = () => {} }: { onChange?: (v: string) => void }) {
  const [v, setV] = useState("dark");
  return (
    <>
      <label htmlFor="theme">Theme</label>
      <Select
        id="theme"
        value={v}
        options={OPTIONS}
        onChange={(x) => {
          setV(x);
          onChange(x);
        }}
      />
      <button type="button">Elsewhere</button>
    </>
  );
}

describe("Select: an in-window dropdown (hidden in screen shares)", () => {
  it("is a labelled combobox showing the value; the list renders inside the page", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const box = screen.getByRole("combobox", { name: "Theme" });
    expect(box).toHaveTextContent("Dark");
    expect(box).toHaveAttribute("aria-expanded", "false");
    expect(document.querySelector("select")).toBeNull(); // no native popup
    await user.click(box);
    const list = screen.getByRole("listbox", { name: "Theme" });
    expect(list).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Dark" })).toHaveAttribute("aria-selected", "true");
  });

  it("picks with the mouse, and disabled options can't be picked", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    const box = screen.getByRole("combobox", { name: "Theme" });
    await choose(user, box, "light");
    expect(box).toHaveTextContent("Light");
    expect(box).toHaveAttribute("data-value", "light");
    await user.click(box);
    await user.click(screen.getByRole("option", { name: "High contrast" }));
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("listbox")).toBeInTheDocument(); // still open
  });

  it("works from the keyboard: arrows skip disabled, Enter picks, Escape closes, letters jump", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    const box = screen.getByRole("combobox", { name: "Theme" });
    box.focus();
    await user.keyboard("{ArrowDown}"); // opens on Dark
    await user.keyboard("{ArrowDown}{ArrowDown}"); // Light, then System (skips disabled)
    expect(box).toHaveAttribute("aria-activedescendant", expect.stringMatching(/-3$/));
    await user.keyboard("{Enter}");
    expect(box).toHaveTextContent("System");
    expect(screen.queryByRole("listbox")).toBeNull();
    await user.keyboard("{Enter}l"); // open, jump to Light
    await user.keyboard(" ");
    expect(box).toHaveTextContent("Light");
    await user.keyboard("{Enter}{Escape}");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(box).toHaveFocus();
  });

  it("closes on an outside click; has no accessibility violations open or closed", async () => {
    const user = userEvent.setup();
    const { container } = render(<Harness />);
    const box = screen.getByRole("combobox", { name: "Theme" });
    expect((await axe.run(container)).violations.map((v) => v.id)).toEqual([]);
    await user.click(box);
    expect((await axe.run(container)).violations.map((v) => v.id)).toEqual([]);
    await user.click(screen.getByRole("button", { name: "Elsewhere" }));
    expect(screen.queryByRole("listbox")).toBeNull();
  });
});
