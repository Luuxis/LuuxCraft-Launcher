/**
 * A neutral 64×64 skin drawn at runtime for accounts without a texture
 * (offline players). Regions follow the standard skin layout.
 */

let cached: HTMLCanvasElement | null = null;

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

function fill(ctx: CanvasRenderingContext2D, color: string, ...rects: Rect[]) {
  ctx.fillStyle = color;
  for (const r of rects) ctx.fillRect(r.x, r.y, r.w, r.h);
}

export function defaultSkinCanvas(): HTMLCanvasElement {
  if (cached) return cached;
  const canvas = document.createElement("canvas");
  canvas.width = 64;
  canvas.height = 64;
  const ctx = canvas.getContext("2d");
  if (!ctx) return canvas;
  ctx.imageSmoothingEnabled = false;

  const skin = "#c99b74";
  const skinDark = "#b3855f";
  const hair = "#3d2b1f";
  const shirt = "#1f8f57";
  const shirtDark = "#166a41";
  const pants = "#2f3e55";
  const pantsDark = "#24303f";
  const shoes = "#3a4757";

  // Head (0,0)-(32,16): top, bottom, right, front, left, back.
  fill(ctx, skin, { x: 0, y: 0, w: 32, h: 16 });
  fill(ctx, hair, { x: 8, y: 0, w: 8, h: 8 }, { x: 0, y: 8, w: 32, h: 3 });
  fill(ctx, skinDark, { x: 16, y: 0, w: 8, h: 8 });
  // Eyes on the front face (8..16, 8..16).
  fill(ctx, "#ffffff", { x: 9, y: 12, w: 2, h: 1 }, { x: 13, y: 12, w: 2, h: 1 });
  fill(ctx, "#22c55e", { x: 10, y: 12, w: 1, h: 1 }, { x: 13, y: 12, w: 1, h: 1 });
  fill(ctx, skinDark, { x: 11, y: 13, w: 2, h: 1 });
  fill(ctx, "#8c5a45", { x: 10, y: 14, w: 4, h: 1 });

  // Body (16,16)-(40,32).
  fill(ctx, shirt, { x: 16, y: 16, w: 24, h: 16 });
  fill(ctx, shirtDark, { x: 16, y: 20, w: 4, h: 12 }, { x: 28, y: 20, w: 4, h: 12 });
  fill(ctx, pants, { x: 20, y: 28, w: 8, h: 4 });

  // Right arm (40,16)-(56,32) and left arm (32,48)-(48,64).
  fill(ctx, shirt, { x: 40, y: 16, w: 16, h: 16 }, { x: 32, y: 48, w: 16, h: 16 });
  fill(ctx, skin, { x: 44, y: 26, w: 4, h: 6 }, { x: 36, y: 58, w: 4, h: 6 }, { x: 40, y: 26, w: 4, h: 6 }, { x: 32, y: 58, w: 4, h: 6 });
  fill(ctx, skin, { x: 48, y: 26, w: 8, h: 6 }, { x: 40, y: 58, w: 8, h: 6 });

  // Right leg (0,16)-(16,32) and left leg (16,48)-(32,64).
  fill(ctx, pants, { x: 0, y: 16, w: 16, h: 16 }, { x: 16, y: 48, w: 16, h: 16 });
  fill(ctx, pantsDark, { x: 0, y: 20, w: 4, h: 12 }, { x: 16, y: 52, w: 4, h: 12 });
  fill(ctx, shoes, { x: 0, y: 29, w: 16, h: 3 }, { x: 16, y: 61, w: 16, h: 3 });

  cached = canvas;
  return canvas;
}

/**
 * Extracts the face (with the hat layer) of a skin as a data URL, scaled with
 * nearest-neighbour so pixels stay crisp.
 */
export async function faceFromSkin(source: string | HTMLCanvasElement, size = 64): Promise<string> {
  const image: CanvasImageSource = await new Promise((resolve, reject) => {
    if (typeof source !== "string") return resolve(source);
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("skin image failed to load"));
    img.src = source;
  });
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (!ctx) return "";
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(image, 8, 8, 8, 8, 0, 0, size, size);
  ctx.drawImage(image, 40, 8, 8, 8, 0, 0, size, size);
  return canvas.toDataURL("image/png");
}
