import { t } from "../../i18n";
import { relativeTime } from "../../lib/format";
import type { Instance } from "../../lib/types";
import { useAppState } from "../../store/AppStore";
import { IconButton } from "../../components/ui/Button";
import { Icon } from "../../components/ui/Icon";
import { Badge, Card, StatusDot } from "../../components/ui/primitives";
import { useServerStatus } from "./useServerStatus";

export function ServerStatusCard({ instance }: { instance: Instance | null }) {
  const { settings } = useAppState();
  const server = instance?.server ?? null;
  const { status, loading, refresh } = useServerStatus(server?.host ?? null, server?.port ?? null, settings?.serverStatusRefreshSeconds ?? 30);

  const label = server?.name || server?.host || instance?.name || "";
  const dotState = !server ? "idle" : status === null ? (loading ? "away" : "idle") : status.online ? "online" : "offline";

  return (
    <Card premium static className="flex flex-col gap-4">
      <div className="flex items-center gap-3">
        {status?.favicon ? (
          <img src={status.favicon} alt="" className="w-9 h-9 rounded-lg pixelated shrink-0" style={{ border: "1px solid var(--border)" }} />
        ) : (
          <span className="icon-chip w-9 h-9 rounded-lg" style={{ borderRadius: 8 }}>
            <Icon name="dns" size={18} />
          </span>
        )}
        <div className="flex-1 min-w-0">
          <p className="text-sm font-bold truncate" style={{ color: "var(--text-primary)" }}>
            {label || t("home.server")}
          </p>
          <p className="text-[11px] font-semibold flex items-center gap-1.5" style={{ color: dotState === "online" ? "#6ee7b7" : dotState === "offline" ? "#fca5a5" : "var(--text-meta)" }}>
            <StatusDot state={dotState} />
            {!server ? t("home.noServer") : status === null ? t("home.checking") : status.online ? t("common.online") : t("home.unreachable")}
          </p>
        </div>
        {server ? <IconButton icon="refresh" label={t("common.refresh")} loading={loading} onClick={refresh} size={16} /> : null}
      </div>
      {server ? (
        <div className="grid grid-cols-2 gap-2">
          <div className="card-inset px-3 py-2.5">
            <p className="text-[10px] font-bold uppercase tracking-wider" style={{ color: "var(--text-meta)" }}>
              {t("home.players")}
            </p>
            <p className="text-xl font-black tabular-nums" style={{ color: "var(--text-primary)" }}>
              {status?.online ? status.players : "—"}
              {status?.online && status.maxPlayers > 0 ? (
                <span className="text-xs font-medium" style={{ color: "var(--text-meta)" }}>
                  {" "}
                  / {status.maxPlayers}
                </span>
              ) : null}
            </p>
          </div>
          <div className="card-inset px-3 py-2.5">
            <p className="text-[10px] font-bold uppercase tracking-wider" style={{ color: "var(--text-meta)" }}>
              {t("home.latency")}
            </p>
            <p className="text-xl font-black tabular-nums" style={{ color: "var(--text-primary)" }}>
              {status?.online && status.latencyMs !== null ? status.latencyMs : "—"}
              {status?.online && status.latencyMs !== null ? (
                <span className="text-xs font-medium" style={{ color: "var(--text-meta)" }}>
                  {" "}
                  ms
                </span>
              ) : null}
            </p>
          </div>
        </div>
      ) : null}
      {status?.online ? (
        <div className="flex items-center gap-2 flex-wrap">
          {status.version ? <Badge variant="neutral">{status.version}</Badge> : null}
          <span className="text-[10px]" style={{ color: "var(--text-placeholder)" }}>
            {t("home.lastCheck", { time: relativeTime(status.checkedAt) })}
          </span>
        </div>
      ) : null}
      {server ? (
        <p className="text-[11px] font-mono truncate selectable" style={{ color: "var(--text-placeholder)" }}>
          {server.host}
          {server.port ? `:${server.port}` : ""}
        </p>
      ) : null}
    </Card>
  );
}
