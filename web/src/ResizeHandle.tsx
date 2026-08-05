// A thin draggable splitter. Reports the incremental pointer delta along one
// axis while dragging, and calls onCommit on release (to persist the new size).
// The viewer's ResizeObserver re-fits the canvas as the neighboring box changes.
import { useRef } from "react";

interface Props {
  axis: "x" | "y";
  onDelta: (delta: number) => void;
  onCommit?: () => void;
  title?: string;
}

export function ResizeHandle({ axis, onDelta, onCommit, title }: Props) {
  const last = useRef(0);

  function onPointerDown(e: React.PointerEvent) {
    e.preventDefault();
    last.current = axis === "x" ? e.clientX : e.clientY;
    const move = (ev: PointerEvent) => {
      const cur = axis === "x" ? ev.clientX : ev.clientY;
      onDelta(cur - last.current);
      last.current = cur;
    };
    const up = () => {
      document.removeEventListener("pointermove", move);
      document.removeEventListener("pointerup", up);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      onCommit?.();
    };
    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", up);
    // Keep the resize cursor and suppress text selection for the whole drag.
    document.body.style.cursor = axis === "x" ? "col-resize" : "row-resize";
    document.body.style.userSelect = "none";
  }

  return (
    <div
      className={`resize-handle resize-${axis}`}
      role="separator"
      aria-orientation={axis === "x" ? "vertical" : "horizontal"}
      title={title}
      onPointerDown={onPointerDown}
    />
  );
}
