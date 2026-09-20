import { useEffect, type ReactNode } from "react";

import { t } from "../../i18n";
import { Button } from "./Button";
import { Icon } from "./Icon";

type Size = "sm" | "md" | "lg";
const SIZES: Record<Size, string> = { sm: "max-w-md", md: "max-w-xl", lg: "max-w-3xl" };

interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  icon?: string;
  size?: Size;
  footer?: ReactNode;
  children: ReactNode;
  /** Prevent closing with Escape / overlay (e.g. while a flow is pending). */
  locked?: boolean;
  /**
   * Renders the body alone, without overlay, header or close button.
   *
   * Used when a theme places this dialog itself: the frame, the title and the
   * backdrop then belong to the theme document, and drawing our own on top
   * would give the player two stacked dialogs.
   */
  inline?: boolean;
}

/** Modal of the charter (§9.8): left brand bar, two radial halos, iconised header. */
export function Modal({ open, onClose, title, subtitle, icon = "edit_square", size = "md", footer, children, locked = false, inline = false }: ModalProps) {
  useEffect(() => {
    if (!open || inline) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !locked) onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, inline, locked, onClose]);

  if (!open) return null;

  if (inline) {
    return (
      <div className="space-y-5" role="group" aria-label={title}>
        {children}
        {footer ? <div className="flex items-center justify-end gap-3 flex-wrap pt-2">{footer}</div> : null}
      </div>
    );
  }

  return (
    <div
      className="modal-overlay"
      role="dialog"
      aria-modal="true"
      aria-labelledby="modal-title"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !locked) onClose();
      }}
    >
      <div className={`relative w-full ${SIZES[size]}`}>
        <button
          type="button"
          className="absolute top-4 right-4 z-20 icon-button"
          style={{ borderRadius: 12 }}
          onClick={onClose}
          disabled={locked}
          aria-label={t("common.close")}
        >
          <Icon name="close" size={20} />
        </button>
        <div className="modal-content">
          <div className="modal-halo-top" />
          <div className="modal-halo-bottom" />
          <header className="modal-header">
            <div className="modal-header-icon">
              <Icon name={icon} size={22} />
            </div>
            <div className="flex-1 min-w-0">
              <h2 id="modal-title" className="text-lg sm:text-xl font-bold leading-tight truncate" style={{ color: "var(--text-primary)" }}>
                {title}
              </h2>
              {subtitle ? (
                <p className="text-[12px] mt-0.5" style={{ color: "var(--text-meta)" }}>
                  {subtitle}
                </p>
              ) : null}
            </div>
          </header>
          <div className="modal-body custom-scrollbar max-h-[70vh] overflow-y-auto space-y-5">{children}</div>
          {footer ? <footer className="modal-footer">{footer}</footer> : null}
        </div>
      </div>
    </div>
  );
}

interface ConfirmProps {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  danger?: boolean;
  loading?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDialog({ open, title, message, confirmLabel, danger = false, loading = false, onConfirm, onCancel }: ConfirmProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onCancel]);

  if (!open) return null;
  return (
    <div
      className="modal-overlay"
      role="alertdialog"
      aria-modal="true"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onCancel();
      }}
    >
      <div
        className="w-full max-w-md rounded-2xl p-6 animate-modal-in"
        style={{
          background: "linear-gradient(to bottom right, var(--bg-primary), var(--bg-deep))",
          border: "1px solid var(--border)",
          boxShadow: "var(--shadow-xl)",
        }}
      >
        <div className="flex items-start gap-4 mb-5">
          <span
            className="w-12 h-12 rounded-xl flex items-center justify-center shrink-0"
            style={{
              background: danger ? "rgba(239,68,68,0.15)" : "rgba(34,197,94,0.2)",
              border: `1px solid ${danger ? "rgba(239,68,68,0.3)" : "rgba(34,197,94,0.3)"}`,
              color: danger ? "#fca5a5" : "#6ee7b7",
            }}
          >
            <Icon name={danger ? "warning" : "help"} size={24} />
          </span>
          <div className="flex-1 min-w-0">
            <h2 className="text-lg font-bold mb-2" style={{ color: "var(--text-primary)" }}>
              {title}
            </h2>
            <p className="text-sm leading-relaxed" style={{ color: "var(--text-label)" }}>
              {message}
            </p>
          </div>
        </div>
        <div className="flex justify-end gap-3">
          <Button variant="ghost" square onClick={onCancel} disabled={loading}>
            {t("common.cancel")}
          </Button>
          <Button variant={danger ? "danger" : "primary"} square onClick={onConfirm} loading={loading}>
            {confirmLabel ?? t("common.confirm")}
          </Button>
        </div>
      </div>
    </div>
  );
}
