import { useState } from "react";

import { describeError, t } from "../../i18n";
import { formatDateTime } from "../../lib/format";
import { loaderIcon, loaderLabel } from "../../lib/instances";
import type { Instance } from "../../lib/types";
import { useActions, useAppState, useInstances, useSelectedAccount } from "../../store/AppStore";
import { Button, IconButton } from "../../components/ui/Button";
import { Select } from "../../components/ui/forms";
import { Badge, Card, Notice } from "../../components/ui/primitives";
import { InstanceSettingsModal } from "../instances/InstanceSettingsModal";
import { LaunchProgress } from "./LaunchProgress";

/** The "Jouer" surface: instance picker, play/install button, running state. */
export function PlayCard() {
  const { game, remote, statuses } = useAppState();
  const { instances, selected, hidden } = useInstances();
  const account = useSelectedAccount();
  const { launch, install, selectInstance, killGame, openLogin, openFolder } = useActions();
  const [editing, setEditing] = useState<Instance | null>(null);

  const maintenance = remote?.config.maintenance ?? false;
  const status = selected ? statuses[selected.id] : undefined;
  const busy = game.mode !== "idle";
  const running = game.running;

  const canPlay = Boolean(account && selected && !busy && !running && !maintenance);

  return (
    <>
      <Card premium static className="space-y-5">
        <div className="flex items-end justify-between gap-3">
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
          {selected ? (
            <div className="flex items-center gap-1.5 shrink-0">
              <IconButton
                icon="folder_open"
                label={t("common.openFolder")}
                onClick={() => void openFolder(`instance:${selected.id}`)}
              />
              <IconButton icon="tune" label={t("instances.settings")} disabled={busy} onClick={() => setEditing(selected)} />
            </div>
          ) : null}
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

        {/* The footer is one block that follows the state — the action row,
            the launch progress, or the running game — never the three stacked.
            A play button greyed out under a progress bar says nothing the bar
            does not, and every extra row was one more reason to scroll. */}
        {busy ? (
          <LaunchProgress />
        ) : running ? (
          <div className="card-inset p-4 flex items-center gap-3 animate-fade-in">
            <span className="status-dot status-dot-online" />
            <div className="flex-1 min-w-0">
              <p className="text-sm font-semibold" style={{ color: "var(--text-primary)" }}>
                {t("home.running")}
              </p>
              <p className="text-[11px] truncate" style={{ color: "var(--text-meta)" }}>
                {running.instanceName || selected?.name}
                {running.startedAt ? ` · ${t("home.runningSince")} ${formatDateTime(running.startedAt)}` : ""}
                {running.pid ? ` · PID ${running.pid}` : ""}
              </p>
            </div>
            <Button variant="danger" size="sm" square icon="stop_circle" onClick={() => void killGame()}>
              {t("home.stop")}
            </Button>
          </div>
        ) : (
          <div className="flex flex-wrap items-center gap-3 pt-1">
            {!account ? (
              <Button variant="primary" size="xl" icon="person_add" onClick={() => openLogin(true)}>
                {t("accounts.add")}
              </Button>
            ) : (
              <Button
                variant="primary"
                size="xl"
                icon="play_arrow"
                disabled={!canPlay}
                onClick={() => selected && void launch(selected.id)}
                className={canPlay ? "animate-pulse-glow" : ""}
              >
                {t("common.play")}
              </Button>
            )}
            {selected ? (
              <Button
                variant="secondary"
                size="lg"
                icon={status?.installed ? "verified" : "download"}
                onClick={() => void install(selected.id)}
              >
                {status?.installed ? t("common.verify") : t("common.install")}
              </Button>
            ) : null}
          </div>
        )}
      </Card>
      {editing ? <InstanceSettingsModal instance={editing} onClose={() => setEditing(null)} /> : null}
    </>
  );
}
