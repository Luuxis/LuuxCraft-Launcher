import { describeError, t } from "../i18n";
import { useActions, useAppState } from "../store/AppStore";
import { TitleBar } from "../components/layout/TitleBar";
import { Sidebar } from "../components/layout/Sidebar";
import { Button } from "../components/ui/Button";
import { Icon } from "../components/ui/Icon";
import { Toasts } from "../components/ui/Toasts";
import { AccountsView } from "../features/accounts/AccountsView";
import { LoginModal } from "../features/accounts/LoginModal";
import { HomeView } from "../features/home/HomeView";
import { SettingsView } from "../features/settings/SettingsView";
import { SkinsView } from "../features/skins/SkinsView";
import { UpdateBanner } from "../features/updater/UpdateBanner";
import type { View } from "../store/AppStore";

/** Views laid out to the window height instead of scrolling with the page. */
const FILLING_VIEWS = new Set<View>(["skins"]);

export function App() {
  const state = useAppState();
  const { refreshRemote, openLogin } = useActions();

  if (state.phase === "booting") {
    return (
      <div className="launcher-bg h-full flex flex-col">
        <TitleBar />
        <div className="relative flex-1 flex flex-col items-center justify-center gap-5 animate-fade-in">
          <div className="blob w-[600px] h-[600px] -top-40 left-1/2 -translate-x-1/2" style={{ background: "rgba(34,197,94,0.08)", filter: "blur(140px)" }} />
          <span className="icon-chip w-16 h-16 rounded-2xl animate-pulse-glow" style={{ borderRadius: 16 }}>
            <Icon name="rocket_launch" size={32} />
          </span>
          <p className="text-sm inline-flex items-center gap-2" style={{ color: "var(--text-body)" }}>
            <Icon name="autorenew" size={16} spin /> {state.bootMessage}
          </p>
        </div>
      </div>
    );
  }

  if (state.phase === "fatal") {
    return (
      <div className="launcher-bg h-full flex flex-col">
        <TitleBar />
        <div className="relative flex-1 flex items-center justify-center p-8">
          <div className="premium-card static p-8 max-w-md w-full space-y-4">
            <span className="icon-chip icon-chip-error w-14 h-14 rounded-2xl" style={{ borderRadius: 16 }}>
              <Icon name="error" size={28} />
            </span>
            <h1 className="text-2xl font-bold" style={{ color: "var(--text-primary)" }}>
              {t("boot.fatalTitle")}
            </h1>
            <p className="text-sm" style={{ color: "var(--text-body)" }}>
              {describeError(state.fatal)}
            </p>
            <p className="text-xs" style={{ color: "var(--text-meta)" }}>
              {t("boot.fatalHint")}
            </p>
            {state.fatal?.details ? (
              <pre className="text-[11px] font-mono whitespace-pre-wrap selectable" style={{ color: "var(--text-placeholder)" }}>
                {state.fatal.details}
              </pre>
            ) : null}
            <Button variant="primary" icon="refresh" onClick={() => window.location.reload()}>
              {t("common.retry")}
            </Button>
          </div>
        </div>
      </div>
    );
  }

  const remote = state.remote;
  const maintenance = remote?.config.maintenance ?? false;

  return (
    <div className="launcher-bg h-full flex flex-col">
      <TitleBar />
      <UpdateBanner />
      {maintenance ? (
        <div className="banner-error px-4 sm:px-6 py-2 flex items-center gap-3 text-sm shrink-0" role="alert">
          <Icon name="engineering" size={18} />
          <span className="font-semibold">{t("boot.maintenanceTitle")}</span>
          <span className="opacity-90 truncate" dangerouslySetInnerHTML={{ __html: (remote?.config.maintenanceMessage ?? "").replace(/<br\s*\/?>/gi, " · ").replace(/<[^>]+>/g, "") }} />
        </div>
      ) : null}
      {remote?.stale || (state.remoteError && !remote) ? (
        <div className="banner-info px-4 sm:px-6 py-2 flex items-center gap-3 text-sm shrink-0" role="status">
          <Icon name="cloud_off" size={18} />
          <span className="font-semibold">{t("boot.offlineTitle")}</span>
          <span className="opacity-90 truncate flex-1">{remote?.stale ? t("boot.offlineText") : describeError(state.remoteError)}</span>
          <Button variant="secondary" size="xs" icon="refresh" loading={state.remoteLoading} onClick={() => void refreshRemote()}>
            {t("common.retry")}
          </Button>
        </div>
      ) : null}
      <div className="relative z-10 flex flex-1 min-h-0">
        <Sidebar />
        {/* Most views scroll with the page; a "filling" one takes the height it
            is given and scrolls inside itself (the skin page, for instance). */}
        <main
          className={`flex-1 min-w-0 overflow-x-hidden custom-scrollbar ${FILLING_VIEWS.has(state.view) ? "overflow-hidden" : "overflow-y-auto"}`}
        >
          <div
            className={`w-full max-w-7xl mx-auto p-4 sm:p-6 lg:p-8 ${FILLING_VIEWS.has(state.view) ? "h-full flex flex-col min-h-0" : ""}`}
          >
            {state.view === "home" ? <HomeView /> : null}
            {state.view === "accounts" ? <AccountsView /> : null}
            {state.view === "skins" ? <SkinsView /> : null}
            {state.view === "settings" ? <SettingsView /> : null}
          </div>
        </main>
      </div>
      <LoginModal open={state.loginOpen} onClose={() => openLogin(false)} />
      <Toasts />
    </div>
  );
}
