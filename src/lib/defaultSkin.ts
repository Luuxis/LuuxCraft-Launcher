/**
 * Le skin par défaut — Steve, tel que le panel le sert aussi à son éditeur —
 * pour tout joueur sans texture connue (compte hors ligne, texture pas encore
 * chargée), et les outils de découpe qui vont avec.
 */

import steve from "../assets/steve.png";

/** Planche 64×64 de Steve, embarquée dans le binaire : disponible hors ligne. */
export const DEFAULT_SKIN_URL: string = steve;

function loadImage(source: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("skin image failed to load"));
    img.src = source;
  });
}

/**
 * Extracts the face (with the hat layer) of a skin as a data URL, scaled with
 * nearest-neighbour so pixels stay crisp.
 */
export async function faceFromSkin(source: string, size = 64): Promise<string> {
  const image = await loadImage(source);
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
