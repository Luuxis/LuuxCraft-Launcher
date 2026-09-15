/**
 * The front panel of a cape, drawn from its texture.
 *
 * A cape texture packs every face; the picker only wants the one you see from
 * behind the player, at (1, 1) for 10×16 pixels in the 64×32 layout. Larger
 * textures (128×64, 512×256…) use the same proportions, so the region is
 * scaled to the texture size.
 */
import { useEffect, useRef } from "react";

/** The front panel, as a fraction of the texture. */
const PANEL = { x: 1 / 64, y: 1 / 32, w: 10 / 64, h: 16 / 32 };

interface CapeImageProps {
  texture: string;
  /** Height of the render in CSS pixels; the width follows the 10×16 panel. */
  height?: number;
  className?: string;
  title?: string;
}

export function CapeImage({ texture, height = 96, className = "", title }: CapeImageProps) {
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const element = canvas.current;
    if (!element) return;
    let cancelled = false;
    const image = new Image();
    image.onload = () => {
      if (cancelled) return;
      const context = element.getContext("2d");
      if (!context) return;
      const scale = 8;
      element.width = 10 * scale;
      element.height = 16 * scale;
      context.clearRect(0, 0, element.width, element.height);
      context.imageSmoothingEnabled = false;
      context.drawImage(
        image,
        PANEL.x * image.width,
        PANEL.y * image.height,
        PANEL.w * image.width,
        PANEL.h * image.height,
        0,
        0,
        element.width,
        element.height,
      );
    };
    image.src = texture;
    return () => {
      cancelled = true;
    };
  }, [texture]);

  return (
    <canvas
      ref={canvas}
      className={`pixelated block rounded-md ${className}`}
      style={{ height, width: (height / 16) * 10 }}
      role="img"
      aria-label={title}
    />
  );
}
