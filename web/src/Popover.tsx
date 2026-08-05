// A small toolbar popover: a trigger button and a panel that opens beneath it,
// closing on outside pointerdown or Escape. Toggles (PopoverToggle) keep it
// open; actions (PopoverAction) close it on activate, so a menu of verbs
// (Project ▾, Export ▾) behaves like a menu while Display's toggles don't.
import {
  createContext,
  useContext,
  useEffect,
  useLayoutEffect,
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
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({
    top: 0,
    left: 0,
    maxHeight: 0,
  });

  // The responsive toolbar scrolls horizontally. A fixed panel escapes that
  // scroll container's overflow clipping while remaining anchored to its
  // trigger, so menus stay above the viewer and clickable on narrow screens.
  useLayoutEffect(() => {
    if (!open) return;

    const place = () => {
      if (!trigger.current || !panel.current) return;
      const triggerRect = trigger.current.getBoundingClientRect();
      const panelWidth = panel.current.getBoundingClientRect().width;
      const viewportInset = 4;
      const top = triggerRect.bottom + viewportInset;
      setPosition({
        top,
        left: Math.max(
          viewportInset,
          Math.min(
            triggerRect.left,
            window.innerWidth - panelWidth - viewportInset,
          ),
        ),
        maxHeight: Math.max(0, window.innerHeight - top - viewportInset),
      });
    };

    place();
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
    };
  }, [open]);

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
        ref={trigger}
        className={`popover-trigger ${active ? "active" : ""}`}
        aria-haspopup="menu"
        aria-expanded={open}
        title={title}
        onClick={() => setOpen((o) => !o)}
      >
        {label} ▾
      </button>
      {open && (
        <div ref={panel} className="popover-panel" role="menu" style={position}>
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
