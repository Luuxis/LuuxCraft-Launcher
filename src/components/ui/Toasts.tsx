import { useActions, useAppState } from "../../store/AppStore";
import { Icon } from "./Icon";

const ICONS = { success: "check_circle", error: "error", warning: "warning", info: "info" };

/** Toasts of the charter (style A: gradient + glass + coloured glow). */
export function Toasts() {
  const { toasts } = useAppState();
  const { dismissToast } = useActions();
  return (
    <div className="alert-container" aria-live="polite">
      {toasts.map((toast) => (
        <div key={toast.id} className={`alert alert-${toast.kind}`} role={toast.kind === "error" ? "alert" : "status"}>
          <Icon name={ICONS[toast.kind]} size={20} className="shrink-0 mt-0.5" />
          <div className="flex-1 min-w-0">
            <p className="font-semibold text-sm">{toast.title}</p>
            {toast.message ? <p className="text-xs mt-0.5 opacity-90 leading-relaxed break-words">{toast.message}</p> : null}
          </div>
          <button
            type="button"
            className="shrink-0 opacity-70 hover:opacity-100 transition-opacity"
            onClick={() => dismissToast(toast.id)}
            aria-label="Fermer"
          >
            <Icon name="close" size={16} />
          </button>
        </div>
      ))}
    </div>
  );
}
