/**
 * Small presentational primitives of the design system: cards, badges,
 * chips, section headers, progress, skeletons, empty states.
 */
import type { HTMLAttributes, ReactNode } from "react";

import { Icon } from "./Icon";

// ── Cards ───────────────────────────────────────────────────────────────────

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  premium?: boolean;
  /** Disable the hover glow (informational surfaces). */
  static?: boolean;
  padding?: string;
}

export function Card({ premium = false, static: isStatic = false, padding = "p-5", className = "", children, ...rest }: CardProps) {
  return (
    <div className={`${premium ? "premium-card" : "card"} ${isStatic ? "static" : ""} ${padding} ${className}`} {...rest}>
      {children}
    </div>
  );
}

// ── Badges ──────────────────────────────────────────────────────────────────

type BadgeVariant = "brand" | "diamond" | "gold" | "error" | "neutral";

interface BadgeProps {
  variant?: BadgeVariant;
  icon?: string;
  dot?: boolean;
  pulse?: boolean;
  children: ReactNode;
  className?: string;
}

export function Badge({ variant = "brand", icon, dot = false, pulse = false, children, className = "" }: BadgeProps) {
  return (
    <span className={`badge badge-${variant} ${className}`}>
      {dot ? (
        <span
          className={`w-1.5 h-1.5 rounded-full ${pulse ? "animate-pulse" : ""}`}
          style={{ background: "currentColor" }}
        />
      ) : null}
      {icon ? <Icon name={icon} size={12} /> : null}
      {children}
    </span>
  );
}

// ── Icon chip ───────────────────────────────────────────────────────────────

interface IconChipProps {
  icon: string;
  size?: "sm" | "md" | "lg" | "xl";
  tone?: "brand" | "diamond" | "gold" | "error" | "pink";
  className?: string;
}

const CHIP_SIZES = { sm: "w-9 h-9 rounded-lg", md: "w-11 h-11 rounded-xl", lg: "w-12 h-12 rounded-xl", xl: "w-20 h-20 rounded-2xl" };
const CHIP_ICON = { sm: 18, md: 22, lg: 24, xl: 40 };

export function IconChip({ icon, size = "md", tone = "brand", className = "" }: IconChipProps) {
  return (
    <span className={`icon-chip ${tone === "brand" ? "" : `icon-chip-${tone}`} ${CHIP_SIZES[size]} ${className}`}>
      <Icon name={icon} size={CHIP_ICON[size]} />
    </span>
  );
}

// ── Section header ──────────────────────────────────────────────────────────

interface SectionHeaderProps {
  icon: string;
  title: string;
  description?: string;
  actions?: ReactNode;
  tone?: IconChipProps["tone"];
}

export function SectionHeader({ icon, title, description, actions, tone }: SectionHeaderProps) {
  return (
    <header
      className="flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 pb-6 mb-6"
      style={{ borderBottom: "1px solid color-mix(in srgb, var(--border) 60%, transparent)" }}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-3 mb-2">
          <IconChip icon={icon} tone={tone} />
          <h2 className="text-2xl sm:text-3xl font-bold tracking-tight" style={{ color: "var(--text-primary)" }}>
            {title}
          </h2>
        </div>
        {description ? (
          <p className="text-sm leading-relaxed" style={{ color: "var(--text-body)" }}>
            {description}
          </p>
        ) : null}
      </div>
      {actions ? <div className="flex flex-wrap items-center gap-2 shrink-0">{actions}</div> : null}
    </header>
  );
}

// ── Progress ────────────────────────────────────────────────────────────────

interface ProgressBarProps {
  value: number;
  indeterminate?: boolean;
  className?: string;
}

export function ProgressBar({ value, indeterminate = false, className = "" }: ProgressBarProps) {
  const width = Math.min(100, Math.max(0, value));
  return (
    <div
      className={`progress ${indeterminate ? "progress-indeterminate" : ""} ${className}`}
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={indeterminate ? undefined : Math.round(width)}
    >
      <div className="progress-fill" style={indeterminate ? undefined : { width: `${width}%` }} />
    </div>
  );
}

// ── Skeleton ────────────────────────────────────────────────────────────────

export function Skeleton({ className = "" }: { className?: string }) {
  return <div className={`skeleton ${className}`} aria-hidden="true" />;
}

// ── Empty state ─────────────────────────────────────────────────────────────

interface EmptyStateProps {
  icon: string;
  title: string;
  description?: string;
  action?: ReactNode;
  compact?: boolean;
}

export function EmptyState({ icon, title, description, action, compact = false }: EmptyStateProps) {
  return (
    <div className={`flex flex-col items-center gap-5 ${compact ? "py-8" : "py-12"}`}>
      <IconChip icon={icon} size={compact ? "lg" : "xl"} />
      <div className="text-center max-w-md">
        <p className="text-base font-semibold" style={{ color: "var(--text-secondary-button)" }}>
          {title}
        </p>
        {description ? (
          <p className="text-sm mt-2 leading-relaxed" style={{ color: "var(--text-meta)" }}>
            {description}
          </p>
        ) : null}
      </div>
      {action ? <div className="mt-1">{action}</div> : null}
    </div>
  );
}

// ── Status dot ──────────────────────────────────────────────────────────────

export function StatusDot({ state }: { state: "online" | "away" | "offline" | "idle" }) {
  return <span className={`status-dot status-dot-${state}`} aria-hidden="true" />;
}

// ── Key/value line ──────────────────────────────────────────────────────────

export function KeyValue({ label, value, mono = false }: { label: string; value: ReactNode; mono?: boolean }) {
  return (
    <div className="flex items-center justify-between gap-4 py-2 text-sm">
      <span style={{ color: "var(--text-meta)" }}>{label}</span>
      <span className={`text-right truncate ${mono ? "font-mono text-xs" : "font-medium"}`} style={{ color: "var(--text-primary)" }}>
        {value}
      </span>
    </div>
  );
}

// ── Inline notice ───────────────────────────────────────────────────────────

interface NoticeProps {
  tone: "success" | "warning" | "error" | "info";
  icon?: string;
  title?: string;
  children?: ReactNode;
  details?: string | null;
  actions?: ReactNode;
}

const NOTICE_STYLES = {
  success: { border: "rgba(34,197,94,0.3)", bg: "rgba(34,197,94,0.1)", color: "#6ee7b7", icon: "verified" },
  warning: { border: "rgba(245,158,11,0.3)", bg: "rgba(245,158,11,0.1)", color: "#fcd34d", icon: "warning" },
  error: { border: "rgba(239,68,68,0.3)", bg: "rgba(239,68,68,0.1)", color: "#fca5a5", icon: "error" },
  info: { border: "rgba(6,182,212,0.3)", bg: "rgba(6,182,212,0.1)", color: "#67e8f9", icon: "info" },
};

export function Notice({ tone, icon, title, children, details, actions }: NoticeProps) {
  const style = NOTICE_STYLES[tone];
  return (
    <div
      className="rounded-2xl px-5 py-4 flex items-start gap-3"
      style={{ border: `1px solid ${style.border}`, background: style.bg }}
      role={tone === "error" ? "alert" : "status"}
    >
      <Icon name={icon ?? style.icon} size={22} className="mt-0.5 shrink-0" style={{ color: style.color }} />
      <div className="flex-1 min-w-0">
        {title ? (
          <p className="text-sm font-semibold" style={{ color: style.color }}>
            {title}
          </p>
        ) : null}
        {children ? (
          <div className="text-sm leading-relaxed mt-0.5" style={{ color: "var(--text-secondary)" }}>
            {children}
          </div>
        ) : null}
        {details ? (
          <details className="mt-2">
            <summary className="text-xs cursor-pointer" style={{ color: "var(--text-meta)" }}>
              Détails techniques
            </summary>
            <pre className="mt-1 text-[11px] font-mono whitespace-pre-wrap break-all selectable" style={{ color: "var(--text-body)" }}>
              {details}
            </pre>
          </details>
        ) : null}
      </div>
      {actions ? <div className="shrink-0 flex items-center gap-2">{actions}</div> : null}
    </div>
  );
}
