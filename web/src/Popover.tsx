// A small toolbar popover: a trigger button and a panel that opens beneath it,
// closing on outside pointerdown or Escape. Used for the Display ▾ menu.
import { useEffect, useRef, useState, type ReactNode } from "react";

interface Props {
  label: ReactNode;
  title?: string;
  /** Highlight the trigger (e.g. a non-default setting is active inside). */
  active?: boolean;
  children: ReactNode;
}

export function Popover({ label, title, active, children }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation(); // don't also dismiss the pick highlight
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", onDown);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("pointerdown", onDown);
      document.removeEventListener("keydown", onKey, true);
    };
  }, [open]);

  return (
    <div className="popover" ref={root}>
      <button
        className={`popover-trigger ${active ? "active" : ""}`}
        aria-haspopup="menu"
        aria-expanded={open}
        title={title}
        onClick={() => setOpen((o) => !o)}
      >
        {label} ▾
      </button>
      {open && <div className="popover-panel">{children}</div>}
    </div>
  );
}

/** A labelled checkbox row for use inside a Popover. */
export function PopoverToggle({
  checked,
  onChange,
  children,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  children: ReactNode;
}) {
  return (
    <label className="popover-row">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span>{children}</span>
    </label>
  );
}
