import { fireEvent, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";

/** Picks `value` in a `Select` (src/lib/Select.tsx): opens it and clicks the option. */
export async function choose(user: UserEvent, combobox: HTMLElement, value: string) {
  await user.click(combobox);
  const listId = combobox.getAttribute("aria-controls") ?? "";
  const list = document.getElementById(listId);
  if (!list) throw new Error("the dropdown didn't open");
  const option = list.querySelector<HTMLElement>(
    `[role="option"][data-value="${CSS.escape(value)}"]`,
  );
  if (!option) throw new Error(`no option with value "${value}"`);
  await user.click(option);
}

/** The options of a `Select`, opened and closed again. */
export async function optionsOf(user: UserEvent, combobox: HTMLElement) {
  await user.click(combobox);
  const list = document.getElementById(combobox.getAttribute("aria-controls") ?? "");
  const options = list ? within(list).getAllByRole("option") : [];
  const out = options.map((o) => ({
    value: o.getAttribute("data-value") ?? "",
    label: o.textContent ?? "",
    disabled: o.getAttribute("aria-disabled") === "true",
  }));
  await user.keyboard("{Escape}");
  return out;
}

/** `choose` without user-event (for tests on fake timers or fireEvent). */
export function pick(combobox: HTMLElement, value: string) {
  fireEvent.click(combobox);
  const list = document.getElementById(combobox.getAttribute("aria-controls") ?? "");
  const option = list?.querySelector<HTMLElement>(
    `[role="option"][data-value="${CSS.escape(value)}"]`,
  );
  if (!option) throw new Error(`no option with value "${value}"`);
  fireEvent.click(option);
}
