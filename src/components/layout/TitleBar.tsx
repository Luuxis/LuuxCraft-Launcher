import { getCurrentWindow } from "@tauri-apps/api/window";

import { t } from "../../i18n";
import { useBrand } from "../../store/AppStore";
import { Icon } from "../ui/Icon";

const windowApi = getCurrentWindow();

/** Custom title bar (decorations are off): drag region + window controls. */
export function TitleBar({ subtitle }: { subtitle?: string }) {
  const brand = useBrand();
  return (
    <header
      data-tauri-drag-region
      className="relative z-30 flex items-center justify-between px-4 shrink-0"
      style={{
        height: "var(--titlebar-height)",
        background: "color-mix(in srgb, var(--bg-deep) 85%, transparent)",
        borderBottom: "1px solid color-mix(in srgb, var(--border) 60%, transparent)",
        backdropFilter: "blur(24px)",
      }}
    >
      <div data-tauri-drag-region className="flex items-center gap-2 min-w-0 pointer-events-none">
        <span className="w-6 h-6 rounded-md flex items-center justify-center shrink-0 icon-chip" style={{ borderRadius: 6 }}>
          <Icon name="rocket_launch" size={14} />
        </span>
        <span className="text-sm font-bold tracking-tight leading-none" style={{ color: "var(--text-primary)" }}>
          {brand.wordmark.prefix}
          <span className="text-gradient">{brand.wordmark.suffix}</span>
        </span>
        <span className="text-[10px] font-bold uppercase tracking-widest text-gradient leading-none ml-1">
          {subtitle ?? brand.subtitle}
        </span>
      </div>
      <div className="flex items-center gap-1">
        <WindowButton icon="remove" label={t("common.minimize")} onClick={() => void windowApi.minimize()} />
        <WindowButton icon="crop_square" label={t("common.maximize")} onClick={() => void windowApi.toggleMaximize()} />
        <WindowButton icon="close" label={t("common.quit")} danger onClick={() => void windowApi.close()} />
      </div>
    </header>
  );
}

function WindowButton({ icon, label, danger = false, onClick }: { icon: string; label: string; danger?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      className={`w-9 h-7 rounded-md flex items-center justify-center transition-all duration-150 ${
        danger ? "hover:bg-redstone-500 hover:text-white" : "hover:bg-white/5"
      }`}
      style={{ color: "var(--text-meta)" }}
      onClick={onClick}
      aria-label={label}
      title={label}
    >
      <Icon name={icon} size={16} />
    </button>
  );
}
