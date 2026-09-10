import { useEffect, useState } from "react";

import { defaultSkinCanvas, faceFromSkin } from "../../lib/defaultSkin";
import { ipc } from "../../lib/ipc";
import type { AccountSummary } from "../../lib/types";
import { initials } from "../../lib/format";

const cache = new Map<string, string>();

interface SkinFaceProps {
  account: AccountSummary;
  size?: number;
  className?: string;
}

/** The player's head, pixel-perfect, from the account textures (or a default). */
export function SkinFace({ account, size = 40, className = "" }: SkinFaceProps) {
  const key = `${account.uuid}:${account.skinUrl ?? account.skinDataUrl?.length ?? "default"}`;
  const [face, setFace] = useState<string | null>(cache.get(key) ?? null);

  useEffect(() => {
    let cancelled = false;
    const cached = cache.get(key);
    if (cached) {
      setFace(cached);
      return;
    }
    (async () => {
      try {
        let source: string | HTMLCanvasElement = defaultSkinCanvas();
        if (account.skinDataUrl) source = account.skinDataUrl;
        else if (account.skinUrl) {
          const data = await ipc.skinGet(account.uuid, false);
          if (data.skin) source = data.skin;
        }
        const url = await faceFromSkin(source, 64);
        cache.set(key, url);
        if (!cancelled) setFace(url);
      } catch {
        if (!cancelled) setFace(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [key, account.uuid, account.skinDataUrl, account.skinUrl]);

  if (!face) {
    return (
      <span
        className={`inline-flex items-center justify-center font-black text-xs shrink-0 ${className}`}
        style={{ width: size, height: size, background: "var(--grad-brand)", color: "#051a0e", borderRadius: 8 }}
        aria-hidden="true"
      >
        {initials(account.name)}
      </span>
    );
  }
  return (
    <img
      src={face}
      alt=""
      width={size}
      height={size}
      className={`pixelated shrink-0 ${className}`}
      style={{ width: size, height: size, borderRadius: className.includes("rounded-full") ? 9999 : 8, border: "1px solid var(--border)" }}
      draggable={false}
    />
  );
}
