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
import { ThemedScreen } from "../theme/ThemedScreen";
import { useTheme } from "../theme/useTheme";
import type { View } from "../store/AppStore";
import type { ScreenId } from "../theme/schema";

/** Views laid out to the window height instead of scrolling with the page. */
const FILLING_VIEWS = new Set<View>(["skins"]);

/** Every view of the app has a screen of the same name in a theme document. */
const SCREEN_OF: Record<View, ScreenId> = {
  home: "home",
  accounts: "accounts",
  skins: "skins",
  settings: "settings",
};

export function App() {
  const state = useAppState();

  // Applies the client's colours to the whole window, and resolves the layout
  // they composed — `null` when they have none, which keeps the built-in one.
  const theme = useTheme();

  if (state.phase === "booting") {
    // The boot screen can be themed too, but only once the panel has answered:
    // before that there is nothing to render it from, and showing the built-in
    // one for a moment beats showing nothing at all.
    if (theme.document) {
      // Standalone: the boot screen carries its own title bar and no menu,
      // since there is nothing to navigate to before the launcher has started.
      return (
        <div className="launcher-bg h-full relative">
          <ThemedScreen document={theme.document} screen="boot" runtime={theme.runtime} shared={false} />
        </div>
      );
    }
    return (
      <div className="launcher-bg h-full flex flex-col">
        <TitleBar />
        <div className="relative flex-1 flex flex-col items-center justify-center gap-5 animate-fade-in">
          <div
            className="blob w-[600px] h-[600px] -top-40 left-1/2 -translate-x-1/2"
            style={{ background: "color-mix(in srgb, var(--accent-500) 8%, transparent)", filter: "blur(140px)" }}
          />
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
    // Deliberately never themed: this screen is what the player sees when the
    // panel is unreachable, which is exactly when a theme cannot be trusted to
    // render. It stays on the engine's own styling.
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

  if (theme.document) return <ThemedApp theme={theme} view={state.view} loginOpen={state.loginOpen} />;

  return <BuiltInApp />;
}

/**
 * The launcher as its owner composed it.
 *
 * Every screen is a layer of the document; the shared layer — title bar, menu,
 * banners — is drawn over each of them. The sign-in screen is an overlay rather
 * than a destination, so it stacks on top of the current one instead of
 * replacing it.
 */
function ThemedApp({ theme, view, loginOpen }: { theme: ReturnType<typeof useTheme>; view: View; loginOpen: boolean }) {
  const document_ = theme.document!;

  return (
    // `launcher-bg` keeps the engine's window background — accent halo and
    // faded grid, both driven by the theme's variables — under whatever the
    // document draws. A screen with no background of its own therefore looks
    // exactly like the built-in layout, which is what the default document is.
    <div
      className="launcher-bg h-full relative overflow-hidden"
      style={{
        // The window has a minimum size of its own, but a theme drawn for a
        // wider one must not be cropped silently: the layer is absolutely
        // positioned and its constraints do the rest.
        minWidth: document_.window.minWidth,
        minHeight: document_.window.minHeight,
      }}
    >
      <ThemedScreen document={document_} screen={SCREEN_OF[view]} runtime={theme.runtime} />
      {loginOpen ? <ThemedScreen document={document_} screen="login" runtime={theme.runtime} shared={false} /> : null}
      <Toasts />
    </div>
  );
}

/** The layout shipped with the engine, used until an owner composes their own. */
function BuiltInApp() {
  const state = useAppState();
  const { refreshRemote, openLogin } = useActions();
  const remote = state.remote;
  const maintenance = remote?.config.maintenance ?? false;

  return (
    <div className="launcher-bg h-full flex flex-col">
      <TitleBar />
      {maintenance ? (
        <div className="banner-error px-4 sm:px-6 py-2 flex items-center gap-3 text-sm shrink-0" role="alert">
          <Icon name="engineering" size={18} />
          <span className="font-semibold">{t("boot.maintenanceTitle")}</span>
          <span
            className="opacity-90 truncate"
            dangerouslySetInnerHTML={{
              __html: (remote?.config.maintenanceMessage ?? "").replace(/<br\s*\/?>/gi, " · ").replace(/<[^>]+>/g, ""),
            }}
          />
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
