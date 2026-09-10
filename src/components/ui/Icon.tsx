import type { CSSProperties } from "react";

interface IconProps {
  name: string;
  size?: number;
  className?: string;
  spin?: boolean;
  style?: CSSProperties;
  label?: string;
}

/** Material Symbols Outlined glyph (self-hosted). */
export function Icon({ name, size = 18, className = "", spin = false, style, label }: IconProps) {
  return (
    <span
      className={`material-symbols-outlined ${spin ? "animate-spin" : ""} ${className}`}
      style={{ fontSize: size, ...style }}
      aria-hidden={label ? undefined : true}
      aria-label={label}
      role={label ? "img" : undefined}
    >
      {name}
    </span>
  );
}

interface BrandIconProps {
  path: string;
  size?: number;
  className?: string;
}

/** Inline 24×24 brand glyph (simple-icons path). */
export function BrandIcon({ path, size = 18, className = "" }: BrandIconProps) {
  return (
    <svg viewBox="0 0 24 24" width={size} height={size} fill="currentColor" className={className} aria-hidden="true">
      <path d={path} />
    </svg>
  );
}
