// ⌘K command palette: a fuzzy-ish filtered list over every web action, so
// visible chrome can stay small without losing discoverability. Web actions
// only — the desktop native menu is a separate Rust-driven surface.
import { useEffect, useMemo, useRef, useState } from "react";

export interface Command {
  id: string;
  title: string;
  /** Keyboard hint shown on the right (display only). */
  shortcut?: string;
  run: () => void;
  /** Hidden from the list when false (e.g. Stop only while rendering). */
  when?: boolean;
}

interface Props {
  commands: Command[];
  onClose: () => void;
}

export function CommandPalette({ commands, onClose }: Props) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const list = useMemo(() => {
    const q = query.trim().toLowerCase();
    const avail = commands.filter((c) => c.when !== false);
    if (!q) return avail;
    return avail.filter((c) => c.title.toLowerCase().includes(q));
  }, [commands, query]);

  // Keep the active index in range as the list shrinks.
  const clamped = Math.min(active, Math.max(0, list.length - 1));

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((i) => Math.min(list.length - 1, i + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => Math.max(0, i - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      const cmd = list[clamped];
      if (cmd) {
        onClose();
        cmd.run();
      }
    }
  }

  return (
    <div className="palette-overlay" onPointerDown={onClose}>
      <div
        className="palette"
        role="dialog"
        aria-label="Command palette"
        onPointerDown={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          className="palette-input"
          placeholder="Type a command…"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
        />
        <ul className="palette-list">
          {list.length === 0 && (
            <li className="palette-empty">No matching command</li>
          )}
          {list.map((c, i) => (
            <li
              key={c.id}
              className={`palette-item ${i === clamped ? "active" : ""}`}
              onPointerEnter={() => setActive(i)}
              onClick={() => {
                onClose();
                c.run();
              }}
            >
              <span>{c.title}</span>
              {c.shortcut && (
                <span className="palette-shortcut">{c.shortcut}</span>
              )}
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
