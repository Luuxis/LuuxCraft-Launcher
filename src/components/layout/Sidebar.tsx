import { useMemo } from "react";

import { t } from "../../i18n";
import { useActions, useAppState, useModules, useSelectedAccount, type View } from "../../store/AppStore";
import { SkinFace } from "../../features/accounts/SkinFace";
import { Icon } from "../ui/Icon";

interface NavModule {
  id: View;
  module: string;
  icon: string;
  label: string;
}

/** The navigation is a registry filtered by the module toggles of the panel. */
const NAV_MODULES: NavModule[] = [
  { id: "home", module: "home", icon: "space_dashboard", label: "nav.home" },
  { id: "accounts", module: "accounts", icon: "manage_accounts", label: "nav.accounts" },
  { id: "skins", module: "skins", icon: "person", label: "nav.skins" },
  { id: "settings", module: "settings", icon: "settings", label: "nav.settings" },
];

export function Sidebar() {
  const { view, game } = useAppState();
  const { navigate, openLogin } = useActions();
  const enabled = useModules();
  const account = useSelectedAccount();

  const items = useMemo(() => NAV_MODULES.filter((item) => item.id === "home" || enabled(item.module)), [enabled]);

  return (
    <aside
      className="flex flex-col w-[232px] shrink-0 overflow-hidden"
      style={{ background: "var(--bg-sidebar)", borderRight: "1px solid color-mix(in srgb, var(--border) 60%, transparent)" }}
      aria-label={t("nav.workspace")}
    >
      <nav className="flex-1 overflow-y-auto overflow-x-hidden custom-scrollbar px-3 py-4">
        <div className="px-2 mb-2">
          <span className="group-label">{t("nav.workspace")}</span>
        </div>
        <ul className="space-y-1">
          {items.map((item) => (
            <li key={item.id}>
              <button
                type="button"
                className={`sidebar-item ${view === item.id ? "active" : ""}`}
                aria-current={view === item.id ? "page" : undefined}
                onClick={() => navigate(item.id)}
              >
                <Icon name={item.icon} size={20} />
                <span className="flex-1">{t(item.label)}</span>
                {item.id === "home" && game.running ? (
                  <span className="w-2 h-2 rounded-full bg-brand-500 shadow-[0_0_8px_#22c55e] animate-pulse" aria-hidden="true" />
                ) : null}
              </button>
            </li>
          ))}
        </ul>
      </nav>
      <div className="p-3" style={{ borderTop: "1px solid color-mix(in srgb, var(--border) 50%, transparent)", background: "var(--bg-footer)" }}>
        {account ? (
          <button
            type="button"
            className="flex items-center gap-2.5 px-2 py-2 rounded-lg w-full text-left transition-colors hover:bg-white/5"
            onClick={() => navigate("accounts")}
          >
            <SkinFace account={account} size={32} className="rounded-full ring-1 ring-inset ring-white/10" />
            <span className="min-w-0 flex-1">
              <span className="block text-xs font-semibold truncate" style={{ color: "var(--text-primary)" }}>
                {account.name}
              </span>
              <span className="block text-[10px] truncate" style={{ color: "var(--text-meta)" }}>
                {t(`accounts.kinds.${account.kind}`)}
              </span>
            </span>
            <span
              className={`w-1.5 h-1.5 rounded-full shrink-0 ${account.needsReauth ? "bg-gold-500 shadow-[0_0_6px_#f59e0b]" : "bg-brand-500 shadow-[0_0_6px_#22c55e]"}`}
              aria-hidden="true"
            />
          </button>
        ) : (
          <button
            type="button"
            className="flex items-center gap-2.5 px-2 py-2 rounded-lg w-full text-left transition-colors hover:bg-white/5"
            onClick={() => openLogin(true)}
          >
            <span className="w-8 h-8 rounded-full flex items-center justify-center icon-chip" style={{ borderRadius: 9999 }}>
              <Icon name="person_add" size={16} />
            </span>
            <span className="min-w-0 flex-1">
              <span className="block text-xs font-semibold" style={{ color: "var(--text-primary)" }}>
                {t("nav.noAccount")}
              </span>
              <span className="block text-[10px]" style={{ color: "var(--text-meta)" }}>
                {t("nav.addAccount")}
              </span>
            </span>
          </button>
        )}
      </div>
    </aside>
  );
}
