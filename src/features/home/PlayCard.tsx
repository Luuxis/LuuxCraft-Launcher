import { describeError, t } from "../../i18n";
import { formatDateTime } from "../../lib/format";
import { loaderIcon, loaderLabel } from "../../lib/instances";
import { useActions, useAppState, useInstances, useSelectedAccount } from "../../store/AppStore";
import { Button } from "../../components/ui/Button";
import { Select } from "../../components/ui/forms";
import { Icon } from "../../components/ui/Icon";
import { Badge, Card, Notice } from "../../components/ui/primitives";
import { LaunchProgress } from "./LaunchProgress";

/** The "Jouer" surface: instance picker, play/install button, running state. */
export function PlayCard() {
  const { game, remote, statuses } = useAppState();
  const { instances, selected, hidden } = useInstances();
  const account = useSelectedAccount();
  const { launch, install, selectInstance, killGame, openLogin, navigate } = useActions();

  const maintenance = remote?.config.maintenance ?? false;
  const status = selected ? statuses[selected.id] : undefined;
  const busy = game.mode !== "idle";
  const running = game.running;
  const runningHere = running && selected && running.instanceId === selected.id;

  const canPlay = Boolean(account && selected && !busy && !running && !maintenance);

  return (
    <Card premium static className="space-y-5">
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 flex-1 space-y-2">
          <span className="group-label">{t("home.instance")}</span>
          <Select
            value={selected?.id ?? null}
            placeholder={instances.length === 0 ? t("home.noInstance") : t("home.chooseInstance")}
            disabled={busy || instances.length === 0}
            onChange={(id) => void selectInstance(id)}
            options={instances.map((instance) => ({
              value: instance.id,
              label: instance.name,
              description: `Minecraft ${instance.minecraftVersion} · ${loaderLabel(instance.loader.kind)}`,
              icon: loaderIcon(instance.loader.kind),
            }))}
          />
        </div>
      </div>

      {selected ? (
        <div className="flex flex-wrap items-center gap-2">
          <Badge variant="neutral" icon="deployed_code">
            <span className="font-mono normal-case tracking-normal">{selected.minecraftVersion}</span>
          </Badge>
          <Badge variant="diamond" icon={loaderIcon(selected.loader.kind)}>
            {loaderLabel(selected.loader.kind)}
            {selected.loader.kind !== "none" && selected.loader.kind !== "mcp" ? ` ${selected.loader.version}` : ""}
          </Badge>
          {status ? (
            status.installed ? (
              <Badge variant="brand" icon="check_circle">
                {t("instances.installed")}
              </Badge>
            ) : (
              <Badge variant="gold" icon="download">
                {t("instances.notInstalled")}
              </Badge>
            )
          ) : null}
          {selected.whitelistActive ? (
            <Badge variant="gold" icon="lock">
              {t("instances.whitelisted")}
            </Badge>
          ) : null}
        </div>
      ) : null}

      {selected?.description ? (
        <p className="text-sm leading-relaxed" style={{ color: "var(--text-body)" }}>
          {selected.description}
        </p>
      ) : null}

      {instances.length === 0 && remote ? (
        <Notice tone="info">
          {t("home.noInstanceHint")}
          {hidden > 0 ? ` ${t("instances.hiddenCount", { count: hidden })}.` : ""}
        </Notice>
      ) : null}

      {game.error ? <Notice tone="error" details={game.error.details ?? game.error.message}>{describeError(game.error)}</Notice> : null}
      {game.lastExit && !game.lastExit.success && game.lastExit.code !== 0 && !game.error && !busy ? (
        <Notice tone="warning">{t("home.crashed", { code: game.lastExit.code ?? "?" })}</Notice>
      ) : null}

      <LaunchProgress />

      {running ? (
        <div className="card-inset p-4 flex items-center gap-3 animate-fade-in">
          <span className="status-dot status-dot-online" />
          <div className="flex-1 min-w-0">
            <p className="text-sm font-semibold" style={{ color: "var(--text-primary)" }}>
              {t("home.running")}
            </p>
            <p className="text-[11px]" style={{ color: "var(--text-meta)" }}>
              {running.instanceName || selected?.name}
              {running.startedAt ? ` · ${t("home.runningSince")} ${formatDateTime(running.startedAt)}` : ""}
              {running.pid ? ` · PID ${running.pid}` : ""}
            </p>
          </div>
          <Button variant="danger" size="sm" square icon="stop_circle" onClick={() => void killGame()}>
            {t("home.stop")}
          </Button>
        </div>
      ) : null}

      <div className="flex flex-wrap items-center gap-3 pt-1">
        {!account ? (
          <Button variant="primary" size="xl" icon="person_add" onClick={() => openLogin(true)}>
            {t("accounts.add")}
          </Button>
        ) : (
          <Button
            variant="primary"
            size="xl"
            icon={runningHere ? "sports_esports" : "play_arrow"}
            disabled={!canPlay}
            loading={busy && game.mode === "launch"}
            onClick={() => selected && void launch(selected.id)}
            className={canPlay ? "animate-pulse-glow" : ""}
          >
            {runningHere ? t("home.running") : t("common.play")}
          </Button>
        )}
        {selected ? (
          <Button
            variant="secondary"
            size="lg"
            icon={status?.installed ? "verified" : "download"}
            disabled={busy || Boolean(running)}
            loading={busy && game.mode === "install"}
            onClick={() => void install(selected.id)}
          >
            {status?.installed ? t("common.verify") : t("common.install")}
          </Button>
        ) : null}
        <span className="flex-1" />
        <button type="button" className="text-xs inline-flex items-center gap-1 transition-colors hover:text-white" style={{ color: "var(--text-meta)" }} onClick={() => navigate("instances")}>
          {t("instances.details")} <Icon name="arrow_forward" size={14} />
        </button>
      </div>
    </Card>
  );
}
