import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

interface Props {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  /** Pair with `<label htmlFor>`; or give `aria-label`. */
  id?: string;
  "aria-label"?: string;
  title?: string;
  /** Classes for the outer box (width, flex). */
  className?: string;
  /** Smaller field, for toolbars. */
  small?: boolean;
  disabled?: boolean;
  /** Called as the list opens (e.g. to refresh the options). */
  onOpen?: () => void;
}

/**
 * A dropdown drawn inside the window (PRD UI-4). A native `<select>` opens its
 * list as a separate OS popup window, which screen-share exclusion doesn't
 * cover, so the options showed up in shared screens. This one renders in the
 * page, so it is hidden with the rest of the overlay.
 *
 * ARIA combobox + listbox: Enter, Space or the arrows open it; the arrows,
 * Home, End and typing a letter move; Enter or Space picks; Escape or Tab closes.
 */
export function Select({
  value,
  options,
  onChange,
  id,
  title,
  className = "",
  small = false,
  disabled = false,
  onOpen,
  ...aria
}: Props) {
  const autoId = useId();
  const buttonId = id ?? `${autoId}-button`;
  const listId = `${autoId}-list`;
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const root = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const selectedIndex = options.findIndex((o) => o.value === value);
  const selected = options[selectedIndex];

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  // Keep the active option in view.
  useEffect(() => {
    if (!open || active < 0) return;
    list.current
      ?.querySelector<HTMLElement>(`[data-index="${active}"]`)
      ?.scrollIntoView?.({ block: "nearest" });
  }, [open, active]);

  const enabled = (i: number) => !!options[i] && !options[i]?.disabled;
  const step = (from: number, dir: 1 | -1) => {
    for (let i = from + dir; i >= 0 && i < options.length; i += dir) if (enabled(i)) return i;
    return from;
  };

  function show() {
    if (disabled) return;
    onOpen?.();
    setActive(selectedIndex >= 0 ? selectedIndex : step(-1, 1));
    setOpen(true);
  }

  function pick(i: number) {
    const o = options[i];
    if (!o || o.disabled) return;
    setOpen(false);
    if (o.value !== value) onChange(o.value);
  }

  function onKeyDown(e: KeyboardEvent<HTMLButtonElement>) {
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setActive((a) => step(a, 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setActive((a) => step(a, -1));
        break;
      case "Home":
        e.preventDefault();
        setActive(step(-1, 1));
        break;
      case "End":
        e.preventDefault();
        setActive(step(options.length, -1));
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        pick(active);
        break;
      case "Escape":
        e.preventDefault();
        setOpen(false);
        break;
      case "Tab":
        setOpen(false);
        break;
      default:
        if (e.key.length === 1 && /\S/.test(e.key)) {
          const k = e.key.toLowerCase();
          const n = options.length;
          for (let j = 1; j <= n; j++) {
            const i = (active + j) % n;
            if (enabled(i) && options[i]?.label.toLowerCase().startsWith(k)) {
              setActive(i);
              break;
            }
          }
        }
    }
  }

  return (
    <div ref={root} className={`relative ${className}`}>
      <button
        type="button"
        id={buttonId}
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={open && active >= 0 ? `${listId}-${active}` : undefined}
        aria-label={aria["aria-label"]}
        title={title}
        data-value={value}
        disabled={disabled}
        className={`field ${small ? "field-sm" : ""} flex w-full items-center gap-1.5 text-left`}
        onClick={() => (open ? setOpen(false) : show())}
        onKeyDown={onKeyDown}
      >
        <span className="min-w-0 flex-1 truncate">{selected?.label ?? ""}</span>
        <span aria-hidden="true" className="text-fg-muted">
          ▾
        </span>
      </button>
      {open && (
        <ul
          ref={list}
          id={listId}
          role="listbox"
          // Named like the field: by its aria-label, or by the button's <label>.
          aria-label={aria["aria-label"]}
          aria-labelledby={aria["aria-label"] ? undefined : buttonId}
          className="absolute left-0 z-40 mt-1 max-h-64 min-w-full overflow-auto rounded-md border border-line bg-popover p-1 text-sm shadow-xl shadow-black/40"
        >
          {options.map((o, i) => (
            // Keyboard use goes through the combobox button (aria-activedescendant).
            // eslint-disable-next-line jsx-a11y/click-events-have-key-events
            <li
              key={o.value}
              id={`${listId}-${i}`}
              data-index={i}
              data-value={o.value}
              role="option"
              aria-selected={o.value === value}
              aria-disabled={o.disabled || undefined}
              className={`cursor-default rounded px-2 py-1 whitespace-nowrap ${
                o.disabled
                  ? "text-fg-muted opacity-60"
                  : i === active
                    ? "bg-surface-3 text-fg"
                    : "text-fg-2"
              } ${o.value === value ? "font-medium" : ""}`}
              // Keep focus on the button so the keyboard keeps working.
              onMouseDown={(e) => e.preventDefault()}
              onMouseEnter={() => !o.disabled && setActive(i)}
              onClick={() => pick(i)}
            >
              {o.label}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
