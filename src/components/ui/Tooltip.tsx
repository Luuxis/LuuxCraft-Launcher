/**
 * Tooltip rendered in a portal, positioned in viewport coordinates.
 *
 * A CSS-only `::after` tooltip is clipped by any ancestor with
 * `overflow: hidden` (every `.premium-card`) and spills outside the window
 * when the trigger sits near an edge. This one is anchored to `document.body`,
 * flips below the trigger when there is no room above, and is clamped to the
 * viewport, so the label always reads inside the visible area.
 */
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type FocusEvent, type MouseEvent } from "react";
import { createPortal } from "react-dom";

/** Space between the trigger and the tooltip. */
const GAP = 8;
/** Minimum distance kept from the window edges. */
const EDGE = 8;

interface Position {
  top: number;
  left: number;
  side: "top" | "bottom";
  /** Arrow offset from the tooltip's left edge, so it keeps pointing at the trigger. */
  arrow: number;
}

function FloatingTooltip({ anchor, label, onDismiss }: { anchor: HTMLElement; label: string; onDismiss: () => void }) {
  const ref = useRef<HTMLSpanElement>(null);
  const [position, setPosition] = useState<Position | null>(null);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const rect = anchor.getBoundingClientRect();
    const width = element.offsetWidth;
    const height = element.offsetHeight;
    const side: Position["side"] = rect.top - height - GAP >= EDGE ? "top" : "bottom";
    const top = side === "top" ? rect.top - height - GAP : rect.bottom + GAP;
    const limit = Math.max(EDGE, window.innerWidth - width - EDGE);
    const left = Math.min(Math.max(EDGE, rect.left + rect.width / 2 - width / 2), limit);
    setPosition({ top, left, side, arrow: rect.left + rect.width / 2 - left });
  }, [anchor, label]);

  // A fixed tooltip would drift away from its trigger: close it instead.
  useEffect(() => {
    window.addEventListener("scroll", onDismiss, true);
    window.addEventListener("resize", onDismiss);
    return () => {
      window.removeEventListener("scroll", onDismiss, true);
      window.removeEventListener("resize", onDismiss);
    };
  }, [onDismiss]);

  return createPortal(
    <span
      ref={ref}
      role="tooltip"
      className={`tooltip-floating ${position ? `tooltip-${position.side}` : ""}`}
      style={
        {
          top: position?.top ?? 0,
          left: position?.left ?? 0,
          // Measured on the first pass, painted on the second.
          visibility: position ? undefined : "hidden",
          "--tooltip-arrow": `${position?.arrow ?? 0}px`,
        } as CSSProperties
      }
    >
      {label}
    </span>,
    document.body,
  );
}

/**
 * Returns the handlers to spread on the trigger and the portal node to render
 * next to it (the node itself lives in `document.body`, so it never affects
 * the trigger's layout).
 */
export function useTooltip(label: string | null | undefined, enabled = true) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const active = enabled && Boolean(label);

  const open = (event: MouseEvent<HTMLElement> | FocusEvent<HTMLElement>) => {
    if (active) setAnchor(event.currentTarget);
  };
  const close = useCallback(() => setAnchor(null), []);

  useEffect(() => {
    if (!active) setAnchor(null);
  }, [active]);

  return {
    triggerProps: {
      onMouseEnter: open,
      onMouseLeave: close,
      onFocus: open,
      onBlur: close,
      // A click means the user acted: the label has served its purpose.
      onPointerDown: close,
    },
    node: active && anchor ? <FloatingTooltip anchor={anchor} label={label!} onDismiss={close} /> : null,
  };
}
