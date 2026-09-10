import { t } from "../../i18n";
import { formatBytes, formatDuration, formatPercent, formatSpeed } from "../../lib/format";
import { useActions, useAppState } from "../../store/AppStore";
import { Button } from "../../components/ui/Button";
import { Icon } from "../../components/ui/Icon";
import { ProgressBar } from "../../components/ui/primitives";

const STAGE_LABELS: Record<string, string> = {
  resolving: "launch.resolving",
  preparing: "launch.preparing",
  checking: "launch.checking",
  downloading: "launch.downloading",
  extracting: "launch.extracting",
  patching: "launch.patching",
  starting: "launch.starting",
};

/** Install/launch progress: stage, bytes, speed, ETA, current element. */
export function LaunchProgress() {
  const { game } = useAppState();
  const { cancelInstall } = useActions();
  if (game.mode === "idle") return null;

  const stage = game.stage ?? "resolving";
  const isDownload = stage === "downloading" && game.progress;
  const isCheck = stage === "checking" && game.check;
  const percent = isDownload ? formatPercent(game.progress!.downloaded, game.progress!.total) : isCheck ? formatPercent(game.check!.checked, game.check!.total) : 0;
  const indeterminate = !isDownload && !isCheck;
  const element = isDownload ? game.progress!.element : isCheck ? game.check!.element : null;
  const cancellable = stage !== "starting";

  return (
    <div className="card-inset p-4 space-y-3 animate-fade-in">
      <div className="flex items-center gap-3">
        <Icon name="autorenew" size={18} spin style={{ color: "#34d399" }} />
        <div className="flex-1 min-w-0">
          <p className="text-sm font-semibold truncate" style={{ color: "var(--text-primary)" }}>
            {t(STAGE_LABELS[stage] ?? "launch.preparing")}
            {!indeterminate ? <span className="font-mono text-xs ml-2" style={{ color: "var(--text-meta)" }}>{percent} %</span> : null}
          </p>
          {element ? (
            <p className="text-[11px] font-mono truncate" style={{ color: "var(--text-meta)" }}>
              {element}
            </p>
          ) : null}
        </div>
        {cancellable ? (
          <Button variant="ghost" size="xs" icon="close" onClick={() => void cancelInstall()}>
            {t("common.cancel")}
          </Button>
        ) : null}
      </div>
      <ProgressBar value={percent} indeterminate={indeterminate} />
      {isDownload ? (
        <div className="grid grid-cols-3 gap-2 text-[11px]" style={{ color: "var(--text-meta)" }}>
          <span className="font-mono tabular-nums">
            {formatBytes(game.progress!.downloaded)} / {formatBytes(game.progress!.total)}
          </span>
          <span className="text-center font-mono tabular-nums">
            {game.speed !== null ? `${t("launch.speed")} ${formatSpeed(game.speed)}` : ""}
          </span>
          <span className="text-right font-mono tabular-nums">
            {game.eta !== null ? `${t("launch.eta")} ${formatDuration(game.eta)}` : ""}
          </span>
        </div>
      ) : null}
      {isCheck ? (
        <p className="text-[11px] font-mono tabular-nums" style={{ color: "var(--text-meta)" }}>
          {t("launch.files")} {game.check!.checked} / {game.check!.total}
        </p>
      ) : null}
    </div>
  );
}
