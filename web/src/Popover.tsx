// A small toolbar popover: a trigger button and a panel that opens beneath it,
// closing on outside pointerdown or Escape. Toggles (PopoverToggle) keep it
// open; actions (PopoverAction) close it on activate, so a menu of verbs
// (Project ▾, Export ▾) behaves like a menu while Display's toggles don't.
import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";

const PopoverCloseCtx = createContext<() => void>(() => {});

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
      {open && (
        <div className="popover-panel" role="menu">
          <PopoverCloseCtx.Provider value={() => setOpen(false)}>
            {children}
          </PopoverCloseCtx.Provider>
        </div>
      )}
    </div>
  );
}

/** A verb row inside a Popover: runs its action and closes the menu. */
export function PopoverAction({
  onClick,
  disabled,
  title,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  title?: string;
  children: ReactNode;
}) {
  const close = useContext(PopoverCloseCtx);
  return (
    <button
      className="popover-row popover-action"
      role="menuitem"
      disabled={disabled}
      title={title}
      onClick={() => {
        close();
        onClick();
      }}
    >
      {children}
    </button>
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
