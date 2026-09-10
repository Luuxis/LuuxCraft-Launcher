import { t } from "../../i18n";
import { formatPercent } from "../../lib/format";
import { useActions, useAppState } from "../../store/AppStore";
import { Button } from "../../components/ui/Button";
import { Icon } from "../../components/ui/Icon";

/** Amber banner (charter §9.21) shown when a launcher update is available. */
export function UpdateBanner() {
  const { update, updateDismissed, updateInstalling, updateProgress, game } = useAppState();
  const { installUpdate, dismissUpdate } = useActions();
  if (!update?.update || updateDismissed) return null;
  const percent = updateProgress?.total ? formatPercent(updateProgress.downloaded, updateProgress.total) : null;
  return (
    <div className="banner-gold px-4 sm:px-6 h-11 flex items-center justify-between gap-3 shrink-0" role="status">
      <div className="flex items-center gap-2 min-w-0 text-sm font-semibold">
        <Icon name="system_update" size={18} />
        <span className="truncate">
          {updateInstalling
            ? percent !== null
              ? t("update.progress", { percent })
              : t("settings.updates.downloading")
            : t("update.banner", { version: update.update.version })}
        </span>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        {!updateInstalling ? (
          <button type="button" className="text-xs font-semibold underline underline-offset-2 opacity-80 hover:opacity-100" onClick={dismissUpdate}>
            {t("update.later")}
          </button>
        ) : null}
        <Button variant="secondary" size="xs" icon="download" loading={updateInstalling} disabled={Boolean(game.running) || game.mode !== "idle"} onClick={() => void installUpdate()}>
          {t("update.install")}
        </Button>
      </div>
    </div>
  );
}
