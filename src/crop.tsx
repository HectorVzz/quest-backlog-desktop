// Crop editor: drag a box over the preview of the whole capture. Only the
// rectangle goes back to Rust, which cuts the full-size image.

import { useEffect, useRef, useState } from "preact/hooks";

import { errorMessage, type Strings } from "./i18n";
import { Icon } from "./icons";
import { native, type CropRect, type ScreenshotView } from "./native";

type Ready = Extract<ScreenshotView, { state: "ready" }>;
type Handle = "nw" | "ne" | "sw" | "se";
type Mode = Handle | "move" | "draw";

const WHOLE: CropRect = { x: 0, y: 0, width: 1, height: 1 };
/** Smallest box kept after a drag, as a fraction of each side. */
const MIN_SIZE = 0.03;
const HANDLES: Handle[] = ["nw", "ne", "sw", "se"];

const clamp = (value: number, min = 0, max = 1) => Math.min(max, Math.max(min, value));

function between(a: { x: number; y: number }, b: { x: number; y: number }): CropRect {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.abs(a.x - b.x),
    height: Math.abs(a.y - b.y),
  };
}

const isWhole = (rect: CropRect) =>
  rect.x <= 0.001 && rect.y <= 0.001 && rect.width >= 0.999 && rect.height >= 0.999;

interface Props {
  t: Strings;
  shot: Ready;
  onClose: () => void;
}

export function CropEditor({ t, shot, onClose }: Props) {
  const [rect, setRect] = useState<CropRect>(shot.crop ?? WHOLE);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const stage = useRef<HTMLDivElement>(null);
  const drag = useRef<{ mode: Mode; start: { x: number; y: number }; origin: CropRect } | null>(null);
  const latest = useRef(rect);
  latest.current = rect;
  const busy = useRef(false);

  const apply = () => {
    if (busy.current) return;
    busy.current = true;
    setSaving(true);
    setError(null);
    const current = latest.current;
    native
      .cropScreenshot(isWhole(current) ? null : current)
      .then(onClose)
      .catch((key) => setError(errorMessage(t, key)))
      .finally(() => {
        busy.current = false;
        setSaving(false);
      });
  };

  // Enter applies, Escape cancels instead of closing the panel.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      } else if (event.key === "Enter") {
        event.preventDefault();
        apply();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  const point = (event: PointerEvent) => {
    const box = stage.current!.getBoundingClientRect();
    return { x: clamp((event.clientX - box.left) / box.width), y: clamp((event.clientY - box.top) / box.height) };
  };

  const onPointerDown = (event: PointerEvent) => {
    if (event.button !== 0) return;
    const start = point(event);
    const handle = (event.target as HTMLElement).dataset.handle as Handle | undefined;
    const inside =
      start.x >= rect.x && start.x <= rect.x + rect.width && start.y >= rect.y && start.y <= rect.y + rect.height;
    const mode: Mode = handle ?? (inside && !isWhole(rect) ? "move" : "draw");
    drag.current = { mode, start, origin: rect };
    stage.current!.setPointerCapture(event.pointerId);
    event.preventDefault();
  };

  const onPointerMove = (event: PointerEvent) => {
    const current = drag.current;
    if (!current) return;
    const p = point(event);
    const { origin, start } = current;
    switch (current.mode) {
      case "draw":
        setRect(between(start, p));
        break;
      case "move":
        setRect({
          ...origin,
          x: clamp(origin.x + p.x - start.x, 0, 1 - origin.width),
          y: clamp(origin.y + p.y - start.y, 0, 1 - origin.height),
        });
        break;
      default: {
        // Resize from a corner: the opposite corner stays put.
        const fixed = {
          x: current.mode.endsWith("w") ? origin.x + origin.width : origin.x,
          y: current.mode.startsWith("n") ? origin.y + origin.height : origin.y,
        };
        setRect(between(fixed, p));
      }
    }
  };

  const onPointerUp = () => {
    const current = drag.current;
    drag.current = null;
    if (!current) return;
    // A click or a tiny box keeps the previous selection.
    const next = latest.current;
    if (next.width < MIN_SIZE || next.height < MIN_SIZE) setRect(current.origin);
  };

  const pixels = `${Math.round(rect.width * shot.width)} × ${Math.round(rect.height * shot.height)}`;
  const percent = (value: number) => `${value * 100}%`;

  return (
    <div class="crop">
      <div class="crop-header">
        <h2>{t.cropTitle}</h2>
        <span class="muted small tabular">{pixels}</span>
      </div>
      <div
        ref={stage}
        class="crop-stage"
        style={`--ratio: ${shot.width / shot.height}`}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
      >
        <img class="crop-image" src={shot.fullPreview} alt={t.screenshotAlt} draggable={false} />
        <div
          class="crop-box"
          data-whole={isWhole(rect)}
          style={{ left: percent(rect.x), top: percent(rect.y), width: percent(rect.width), height: percent(rect.height) }}
        >
          {HANDLES.map((handle) => (
            <span key={handle} class={`crop-handle crop-${handle}`} data-handle={handle} />
          ))}
        </div>
      </div>
      <p class="muted small">{t.cropHint}</p>
      {error && (
        <p class="error" role="alert">
          {error}
        </p>
      )}
      <div class="actions">
        <button data-primary class="button primary" disabled={saving} onClick={apply}>
          <Icon name="check" size={16} />
          {t.applyCrop}
        </button>
        <button class="button subtle" disabled={saving || isWhole(rect)} onClick={() => setRect(WHOLE)}>
          {t.resetCrop}
        </button>
        <button class="button subtle" disabled={saving} onClick={onClose}>
          {t.cancel}
        </button>
      </div>
    </div>
  );
}
