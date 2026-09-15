/**
 * Front view of a skin, drawn on a canvas from the PNG texture.
 *
 * The library shows a dozen skins at once: one WebGL context per tile would be
 * wasteful, so the tiles use this flat render (head, torso, arms, legs plus
 * their overlay layers) and only the "Actuel" panel keeps the 3D viewer.
 */
import { useEffect, useRef } from "react";

import type { SkinVariant } from "../../lib/types";

/** Source rectangles of the 64×64 layout, in texture pixels. */
interface Part {
  /** Source x, y, then destination x — widths come from the part itself. */
  sx: number;
  sy: number;
  w: number;
  h: number;
  dx: number;
  dy: number;
  /** Legacy 64×32 skins only carry the right half: mirror it. */
  mirrorOfRight?: boolean;
}

/** The drawing grid: 16 skin-pixels wide, 32 tall, torso centred on x = 4. */
const GRID_WIDTH = 16;
const GRID_HEIGHT = 32;

function parts(armWidth: number): { base: Part[]; overlay: Part[] } {
  const leftArmX = 4 - armWidth;
  return {
    base: [
      { sx: 8, sy: 8, w: 8, h: 8, dx: 4, dy: 0 }, // head
      { sx: 20, sy: 20, w: 8, h: 12, dx: 4, dy: 8 }, // torso
      { sx: 44, sy: 20, w: armWidth, h: 12, dx: leftArmX, dy: 8 }, // right arm
      { sx: 36, sy: 52, w: armWidth, h: 12, dx: 12, dy: 8, mirrorOfRight: true }, // left arm
      { sx: 4, sy: 20, w: 4, h: 12, dx: 4, dy: 20 }, // right leg
      { sx: 20, sy: 52, w: 4, h: 12, dx: 8, dy: 20, mirrorOfRight: true }, // left leg
    ],
    overlay: [
      { sx: 40, sy: 8, w: 8, h: 8, dx: 4, dy: 0 }, // hat
      { sx: 20, sy: 36, w: 8, h: 12, dx: 4, dy: 8 }, // jacket
      { sx: 44, sy: 36, w: armWidth, h: 12, dx: leftArmX, dy: 8 }, // right sleeve
      { sx: 52, sy: 52, w: armWidth, h: 12, dx: 12, dy: 8 }, // left sleeve
      { sx: 4, sy: 36, w: 4, h: 12, dx: 4, dy: 20 }, // right trouser
      { sx: 4, sy: 52, w: 4, h: 12, dx: 8, dy: 20 }, // left trouser
    ],
  };
}

/** Mirror of the right limb, for the 64×32 layout that has no left one. */
const LEGACY_MIRROR: Record<number, { sx: number; sy: number }> = {
  36: { sx: 44, sy: 20 }, // left arm ← right arm
  20: { sx: 4, sy: 20 }, // left leg ← right leg
};

function draw(canvas: HTMLCanvasElement, image: HTMLImageElement, slim: boolean, scale: number) {
  const context = canvas.getContext("2d");
  if (!context) return;
  const legacy = image.height < 64;
  const { base, overlay } = parts(slim ? 3 : 4);

  canvas.width = GRID_WIDTH * scale;
  canvas.height = GRID_HEIGHT * scale;
  context.clearRect(0, 0, canvas.width, canvas.height);
  context.imageSmoothingEnabled = false;

  const paint = (part: Part) => {
    let { sx, sy } = part;
    if (legacy && part.mirrorOfRight) {
      const mirror = LEGACY_MIRROR[part.sx];
      if (!mirror) return;
      ({ sx, sy } = mirror);
      // Flip horizontally so the limb faces the right way.
      context.save();
      context.translate((part.dx + part.w) * scale, 0);
      context.scale(-1, 1);
      context.drawImage(image, sx, sy, part.w, part.h, 0, part.dy * scale, part.w * scale, part.h * scale);
      context.restore();
      return;
    }
    context.drawImage(image, sx, sy, part.w, part.h, part.dx * scale, part.dy * scale, part.w * scale, part.h * scale);
  };

  base.forEach(paint);
  // Legacy skins only have the hat overlay.
  (legacy ? overlay.slice(0, 1) : overlay).forEach(paint);
}

interface SkinBodyProps {
  texture: string;
  variant?: SkinVariant;
  /** Height of the render in CSS pixels; the width follows the 16×32 grid. */
  height?: number;
  className?: string;
  title?: string;
}

export function SkinBody({ texture, variant = "classic", height = 120, className = "", title }: SkinBodyProps) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    let cancelled = false;
    const image = new Image();
    image.onload = () => {
      if (!cancelled) draw(element, image, variant === "slim", 8);
    };
    image.src = texture;
    return () => {
      cancelled = true;
    };
  }, [texture, variant]);

  return (
    <canvas
      ref={canvas}
      className={`pixelated block ${className}`}
      style={{ height, width: (height / GRID_HEIGHT) * GRID_WIDTH }}
      role="img"
      aria-label={title}
    />
  );
}
