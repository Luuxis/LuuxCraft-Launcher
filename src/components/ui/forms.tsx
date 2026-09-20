/**
 * Form controls: labelled fields, inputs, password, custom select, slider,
 * toggle and option cards. Visuals follow the charter (§9.2).
 */
import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState, type InputHTMLAttributes, type ReactNode } from "react";
import { createPortal } from "react-dom";

import { Icon } from "./Icon";

// ── Field ───────────────────────────────────────────────────────────────────

interface FieldProps {
  label: string;
  icon?: string;
  hint?: string;
  htmlFor?: string;
  trailing?: ReactNode;
  children: ReactNode;
}

export function Field({ label, icon, hint, htmlFor, trailing, children }: FieldProps) {
  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-3">
        <label htmlFor={htmlFor} className="field-label">
          {icon ? <Icon name={icon} size={18} /> : null}
          {label}
        </label>
        {trailing}
      </div>
      {children}
      {hint ? (
        <p className="text-xs leading-relaxed" style={{ color: "var(--text-meta)" }}>
          {hint}
        </p>
      ) : null}
    </div>
  );
}

// ── Input ───────────────────────────────────────────────────────────────────

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  mono?: boolean;
}

export function Input({ mono = false, className = "", ...rest }: InputProps) {
  return <input className={`input-premium ${mono ? "mono" : ""} ${className}`} spellCheck={false} {...rest} />;
}

export function PasswordInput({ className = "", ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  const [visible, setVisible] = useState(false);
  return (
    <div className="relative">
      <input type={visible ? "text" : "password"} className={`input-premium pr-12 ${className}`} spellCheck={false} {...rest} />
      <button
        type="button"
        className="absolute inset-y-0 right-0 px-4 flex items-center transition-colors"
        style={{ color: "var(--text-meta)" }}
        onClick={() => setVisible((v) => !v)}
        aria-label={visible ? "Masquer le mot de passe" : "Afficher le mot de passe"}
        tabIndex={-1}
      >
        <Icon name={visible ? "visibility_off" : "visibility"} size={20} />
      </button>
    </div>
  );
}

// ── Textarea ────────────────────────────────────────────────────────────────

export function Textarea({ className = "", ...rest }: React.TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea className={`input-premium mono min-h-24 resize-y ${className}`} spellCheck={false} {...rest} />;
}

// ── Select ──────────────────────────────────────────────────────────────────

export interface SelectOption<T extends string> {
  value: T;
  label: string;
  description?: string;
  icon?: string;
}

interface SelectProps<T extends string> {
  value: T | null;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  placeholder?: string;
  disabled?: boolean;
  id?: string;
  className?: string;
}

/** Space between the trigger and its dropdown, and margin kept from the window edges. */
const DROPDOWN_GAP = 6;
const DROPDOWN_EDGE = 12;
const DROPDOWN_MAX_HEIGHT = 256;

interface DropdownBox {
  left: number;
  width: number;
  top?: number;
  bottom?: number;
  maxHeight: number;
}

function measureDropdown(trigger: HTMLElement): DropdownBox {
  const rect = trigger.getBoundingClientRect();
  const below = window.innerHeight - rect.bottom - DROPDOWN_GAP - DROPDOWN_EDGE;
  const above = rect.top - DROPDOWN_GAP - DROPDOWN_EDGE;
  // Open upwards only when that genuinely leaves more room.
  const up = below < 180 && above > below;
  return {
    left: rect.left,
    width: rect.width,
    top: up ? undefined : rect.bottom + DROPDOWN_GAP,
    bottom: up ? window.innerHeight - rect.top + DROPDOWN_GAP : undefined,
    maxHeight: Math.max(120, Math.min(DROPDOWN_MAX_HEIGHT, up ? above : below)),
  };
}

export function Select<T extends string>({ value, options, onChange, placeholder, disabled, id, className = "" }: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const [box, setBox] = useState<DropdownBox | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const dropdown = useRef<HTMLUListElement>(null);
  const listId = useId();
  const current = options.find((option) => option.value === value) ?? null;

  // The list is portalled to `document.body` so a card with `overflow: hidden`
  // can no longer clip it — it stays fully visible and scrollable.
  const reposition = useCallback(() => {
    if (trigger.current) setBox(measureDropdown(trigger.current));
  }, []);

  useLayoutEffect(() => {
    if (open) reposition();
    else setBox(null);
  }, [open, reposition]);

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (trigger.current?.contains(target) || dropdown.current?.contains(target)) return;
      setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    // Follow the trigger when the page behind scrolls (the dropdown itself is
    // excluded, otherwise scrolling the list would fight with repositioning).
    const onScroll = (event: Event) => {
      if (dropdown.current?.contains(event.target as Node)) return;
      reposition();
    };
    document.addEventListener("mousedown", onPointerDown);
    document.addEventListener("keydown", onKey);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", reposition);
    return () => {
      document.removeEventListener("mousedown", onPointerDown);
      document.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", reposition);
    };
  }, [open, reposition]);

  return (
    <div className={`relative ${className}`}>
      <button
        ref={trigger}
        type="button"
        id={id}
        className="select-trigger"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="flex items-center gap-2.5 min-w-0">
          {current?.icon ? <Icon name={current.icon} size={18} style={{ color: "var(--brand-primary)" }} /> : null}
          <span className={`truncate ${current ? "" : "opacity-60"}`}>{current?.label ?? placeholder ?? "—"}</span>
        </span>
        <Icon
          name="expand_more"
          size={20}
          className={`transition-all duration-200 ${open ? "rotate-180" : ""}`}
          style={{ color: open ? "var(--brand-primary)" : "var(--text-meta)" }}
        />
      </button>
      {open && box
        ? createPortal(
            <ul
              ref={dropdown}
              id={listId}
              role="listbox"
              className="select-dropdown custom-scrollbar"
              style={{ left: box.left, width: box.width, top: box.top, bottom: box.bottom, maxHeight: box.maxHeight }}
            >
              {options.length === 0 ? (
                <li className="py-5 px-3 text-center text-[13px]" style={{ color: "var(--text-meta)" }}>
                  —
                </li>
              ) : null}
              {options.map((option) => (
                <li key={option.value} role="option" aria-selected={option.value === value}>
                  <button
                    type="button"
                    className="select-option"
                    aria-selected={option.value === value}
                    onClick={() => {
                      onChange(option.value);
                      setOpen(false);
                    }}
                  >
                    <span
                      className="w-4 h-4 rounded-md border flex items-center justify-center shrink-0"
                      style={{
                        background: option.value === value ? "var(--brand-primary)" : "transparent",
                        borderColor: option.value === value ? "var(--brand-primary)" : "var(--border)",
                        color: option.value === value ? "#ffffff" : "transparent",
                      }}
                    >
                      <Icon name="check" size={12} />
                    </span>
                    {option.icon ? <Icon name={option.icon} size={18} style={{ color: "var(--text-meta)" }} /> : null}
                    <span className="flex-1 min-w-0">
                      <span className="block truncate">{option.label}</span>
                      {option.description ? (
                        <span className="block text-[11px] truncate" style={{ color: "var(--text-meta)" }}>
                          {option.description}
                        </span>
                      ) : null}
                    </span>
                  </button>
                </li>
              ))}
            </ul>,
            document.body,
          )
        : null}
    </div>
  );
}

// ── Slider ──────────────────────────────────────────────────────────────────

interface SliderProps {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
  format?: (value: number) => string;
  disabled?: boolean;
  id?: string;
}

export function Slider({ value, min, max, step = 1, onChange, onCommit, format, disabled, id }: SliderProps) {
  const progress = max > min ? ((value - min) / (max - min)) * 100 : 0;
  return (
    <div className="flex items-center gap-4">
      <input
        id={id}
        type="range"
        className="slider flex-1"
        style={{ "--progress": `${progress}%` } as React.CSSProperties}
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(Number(event.target.value))}
        onMouseUp={(event) => onCommit?.(Number((event.target as HTMLInputElement).value))}
        onKeyUp={(event) => onCommit?.(Number((event.target as HTMLInputElement).value))}
        onTouchEnd={(event) => onCommit?.(Number((event.target as HTMLInputElement).value))}
      />
      <span className="font-mono text-xs w-20 text-right tabular-nums" style={{ color: "var(--text-primary)" }}>
        {format ? format(value) : value}
      </span>
    </div>
  );
}

// ── Toggle ──────────────────────────────────────────────────────────────────

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
}

export function Toggle({ checked, onChange, label, description, disabled }: ToggleProps) {
  return (
    <label className={`flex items-center justify-between gap-4 py-2 ${disabled ? "opacity-50" : "cursor-pointer"}`}>
      <span className="min-w-0">
        <span className="block text-sm font-medium" style={{ color: "var(--text-label)" }}>
          {label}
        </span>
        {description ? (
          <span className="block text-xs mt-0.5 leading-relaxed" style={{ color: "var(--text-meta)" }}>
            {description}
          </span>
        ) : null}
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={label}
        className="toggle"
        disabled={disabled}
        onClick={() => onChange(!checked)}
      />
    </label>
  );
}

// ── Option card (radio group item) ──────────────────────────────────────────

interface OptionCardProps {
  checked: boolean;
  onSelect: () => void;
  icon: string;
  title: string;
  description?: string;
  disabled?: boolean;
}

export function OptionCard({ checked, onSelect, icon, title, description, disabled }: OptionCardProps) {
  return (
    <button type="button" role="radio" aria-checked={checked} className="option-card" onClick={onSelect} disabled={disabled}>
      <span
        className="w-9 h-9 rounded-lg flex items-center justify-center shrink-0"
        style={{
          background: checked ? "color-mix(in srgb, var(--accent-500) 15%, transparent)" : "var(--bg-secondary)",
          border: `1px solid ${checked ? "color-mix(in srgb, var(--accent-500) 35%, transparent)" : "var(--border)"}`,
          color: checked ? "var(--accent-400)" : "var(--text-meta)",
        }}
      >
        <Icon name={icon} size={18} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block text-sm font-semibold" style={{ color: "var(--text-primary)" }}>
          {title}
        </span>
        {description ? (
          <span className="block text-xs mt-0.5 leading-relaxed" style={{ color: "var(--text-meta)" }}>
            {description}
          </span>
        ) : null}
      </span>
      <span
        className="w-5 h-5 rounded-full border flex items-center justify-center shrink-0 mt-0.5"
        style={{
          borderColor: checked ? "var(--brand-primary)" : "var(--border)",
          background: checked ? "var(--brand-primary)" : "transparent",
          color: "var(--on-brand)",
        }}
      >
        {checked ? <Icon name="check" size={14} /> : null}
      </span>
    </button>
  );
}
