import type { ButtonHTMLAttributes, ReactNode } from "react";

import { Icon } from "./Icon";
import { useTooltip } from "./Tooltip";

type Variant = "primary" | "secondary" | "ghost" | "danger" | "gold";
type Size = "xs" | "sm" | "md" | "lg" | "xl";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  icon?: string;
  iconRight?: string;
  loading?: boolean;
  /** Rounded corners instead of a pill (compact contexts). */
  square?: boolean;
  children?: ReactNode;
}

const SIZES: Record<Size, string> = {
  xs: "h-8 px-3 text-xs gap-1.5",
  sm: "h-9 px-4 text-xs",
  md: "h-10 px-5 text-sm",
  lg: "h-12 px-6 text-sm",
  xl: "h-13 px-7 text-base",
};

const ICON_SIZES: Record<Size, number> = { xs: 16, sm: 16, md: 18, lg: 18, xl: 20 };

export function Button({
  variant = "secondary",
  size = "md",
  icon,
  iconRight,
  loading = false,
  square = false,
  className = "",
  children,
  disabled,
  type = "button",
  ...rest
}: ButtonProps) {
  const iconSize = ICON_SIZES[size];
  return (
    <button
      type={type}
      className={`btn-pill btn-pill-${variant} ${SIZES[size]} ${square ? "btn-square" : ""} ${className}`}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading ? <Icon name="autorenew" size={iconSize} spin /> : icon ? <Icon name={icon} size={iconSize} /> : null}
      {children ? <span className="relative">{children}</span> : null}
      {iconRight && !loading ? <Icon name={iconRight} size={iconSize} /> : null}
    </button>
  );
}

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: string;
  label: string;
  danger?: boolean;
  size?: number;
  loading?: boolean;
  tooltip?: boolean;
}

export function IconButton({
  icon,
  label,
  danger = false,
  size = 18,
  loading = false,
  tooltip = true,
  className = "",
  disabled,
  type = "button",
  ...rest
}: IconButtonProps) {
  const { triggerProps, node } = useTooltip(label, tooltip);
  return (
    <>
      <button
        type={type}
        className={`icon-button ${danger ? "danger" : ""} ${className}`}
        aria-label={label}
        title={tooltip ? undefined : label}
        disabled={disabled || loading}
        {...rest}
        {...triggerProps}
      >
        <Icon name={loading ? "autorenew" : icon} size={size} spin={loading} />
      </button>
      {node}
    </>
  );
}
