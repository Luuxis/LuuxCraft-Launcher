import { useEffect, useState } from "react";

import { t } from "../../i18n";
import { ipc } from "../../lib/ipc";
import { loaderIcon, loaderLabel } from "../../lib/instances";
import type { Instance, JavaRequirement } from "../../lib/types";
import { useActions, useAppState, useInstances, useSelectedAccount } from "../../store/AppStore";
import { Button, IconButton } from "../../components/ui/Button";
import { Icon } from "../../components/ui/Icon";
import { Badge, Card, EmptyState, KeyValue, Notice, SectionHeader } from "../../components/ui/primitives";
import { LaunchProgress } from "../home/LaunchProgress";
import { InstanceSettingsModal } from "./InstanceSettingsModal";

export function InstancesView() {
  const { remoteLoading, statuses, game, settings } = useAppState();
  const { instances, hidden, selected } = useInstances();
  const account = useSelectedAccount();
  const { refreshRemote, selectInstance, install, launch, openFolder } = useActions();
  const [detailsId, setDetailsId] = useState<string | null>(selected?.id ?? null);
  const [editing, setEditing] = useState<Instance | null>(null);

  useEffect(() => {
    if (!detailsId && selected) setDetailsId(selected.id);
  }, [detailsId, selected]);

  const details = instances.find((i) => i.id === detailsId) ?? selected;
  const busy = game.mode !== "idle";

  return (
    <div className="space-y-6 animate-fade-in-up">
      <SectionHeader
        icon="tune"
        title={t("instances.title")}
        description={t("instances.subtitle")}
        actions={
          <Button variant="secondary" icon="refresh" loading={remoteLoading} onClick={() => void refreshRemote()}>
            {t("common.refresh")}
          </Button>
        }
      />

      {hidden > 0 ? (
        <Notice tone="info" icon="lock">
          {t("instances.hiddenCount", { count: hidden })}
        </Notice>
      ) : null}

      {instances.length === 0 ? (
        <Card static>
          <EmptyState icon="inbox" title={t("instances.empty")} description={t("instances.emptyHint")} />
        </Card>
      ) : (
        <div className="grid grid-cols-1 xl:grid-cols-[minmax(0,1fr)_380px] gap-5 items-start">
          <ul className="space-y-3">
            {instances.map((instance, index) => {
              const status = statuses[instance.id];
              const active = details?.id === instance.id;
              const isSelected = settings?.selectedInstance === instance.id || (!settings?.selectedInstance && selected?.id === instance.id);
              const running = game.running?.instanceId === instance.id;
              return (
                <li key={instance.id} className={`animate-fade-in-up stagger-${Math.min(index + 1, 5)}`} style={{ opacity: 0 }}>
                  <div
                    role="button"
                    tabIndex={0}
                    onClick={() => setDetailsId(instance.id)}
                    onKeyDown={(e) => e.key === "Enter" && setDetailsId(instance.id)}
                    className="flex items-center gap-4 p-3 rounded-xl cursor-pointer transition-all duration-200"
                    style={{
                      background: active ? "rgba(34,197,94,0.08)" : "color-mix(in srgb, var(--bg-deep) 60%, transparent)",
                      border: `1px solid ${active ? "rgba(34,197,94,0.45)" : "color-mix(in srgb, var(--border) 60%, transparent)"}`,
                      boxShadow: active ? "var(--glow-sm)" : undefined,
                    }}
                  >
                    {instance.image ? (
                      <img src={instance.image} alt="" className="w-12 h-12 rounded-lg object-cover shrink-0" style={{ border: "1px solid var(--border)" }} />
                    ) : (
                      <span className="icon-chip w-12 h-12 rounded-lg shrink-0" style={{ borderRadius: 8 }}>
                        <Icon name={loaderIcon(instance.loader.kind)} size={22} />
                      </span>
                    )}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className={`w-2 h-2 rounded-full shrink-0 ${running ? "bg-brand-500 shadow-[0_0_8px_#22c55e] animate-pulse" : status?.installed ? "bg-brand-500 shadow-[0_0_8px_#22c55e]" : "bg-obsidian-400"}`} />
                        <span className="text-sm font-semibold truncate" style={{ color: "var(--text-primary)" }}>
                          {instance.name}
                        </span>
                        {isSelected ? <Badge variant="brand">{t("common.select")}</Badge> : null}
                        {instance.whitelistActive ? (
                          <Badge variant="gold" icon="lock">
                            {t("instances.whitelisted")}
                          </Badge>
                        ) : null}
                      </div>
                      {instance.description ? (
                        <p className="text-xs truncate mt-0.5" style={{ color: "var(--text-meta)" }}>
                          {instance.description}
                        </p>
                      ) : null}
                    </div>
                    <span className="text-xs font-mono shrink-0" style={{ color: "var(--text-meta)" }}>
                      {instance.minecraftVersion}
                    </span>
                    <span className="badge badge-neutral shrink-0">{loaderLabel(instance.loader.kind)}</span>
                  </div>
                </li>
              );
            })}
          </ul>

          {details ? (
            <InstanceDetails
              instance={details}
              installed={statuses[details.id]?.installed ?? false}
              busy={busy}
              running={Boolean(game.running)}
              hasAccount={Boolean(account)}
              onPlay={() => void launch(details.id)}
              onInstall={() => void install(details.id)}
              onSelect={() => void selectInstance(details.id)}
              onOpen={() => void openFolder(`instance:${details.id}`)}
              onEdit={() => setEditing(details)}
              selectedId={settings?.selectedInstance ?? selected?.id ?? null}
            />
          ) : null}
        </div>
      )}

      {editing ? <InstanceSettingsModal instance={editing} onClose={() => setEditing(null)} /> : null}
    </div>
  );
}

interface DetailsProps {
  instance: Instance;
  installed: boolean;
  busy: boolean;
  running: boolean;
  hasAccount: boolean;
  selectedId: string | null;
  onPlay: () => void;
  onInstall: () => void;
  onSelect: () => void;
  onOpen: () => void;
  onEdit: () => void;
}

function InstanceDetails({ instance, installed, busy, running, hasAccount, selectedId, onPlay, onInstall, onSelect, onOpen, onEdit }: DetailsProps) {
  const [java, setJava] = useState<JavaRequirement | null>(null);
  const [javaError, setJavaError] = useState(false);
  const { game, settings } = useAppState();

  useEffect(() => {
    let cancelled = false;
    setJava(null);
    setJavaError(false);
    ipc
      .javaRequired(instance.id)
      .then((req) => !cancelled && setJava(req))
      .catch(() => !cancelled && setJavaError(true));
    return () => {
      cancelled = true;
    };
  }, [instance.id]);

  const overrides = settings?.instances[instance.id];
  const hasOverrides = Boolean(overrides && (overrides.memory || overrides.java || overrides.jvmArgs));

  return (
    <Card premium static className="space-y-4 xl:sticky xl:top-0">
      <div className="flex items-start gap-3">
        {instance.image ? (
          <img src={instance.image} alt="" className="w-14 h-14 rounded-xl object-cover shrink-0" style={{ border: "1px solid var(--border)" }} />
        ) : (
          <span className="icon-chip w-14 h-14 rounded-xl shrink-0">
            <Icon name={loaderIcon(instance.loader.kind)} size={26} />
          </span>
        )}
        <div className="min-w-0 flex-1">
          <h3 className="text-lg font-bold truncate" style={{ color: "var(--text-primary)" }}>
            {instance.name}
          </h3>
          <div className="flex flex-wrap gap-1.5 mt-1">
            {installed ? (
              <Badge variant="brand" icon="check_circle">
                {t("instances.installed")}
              </Badge>
            ) : (
              <Badge variant="gold" icon="download">
                {t("instances.notInstalled")}
              </Badge>
            )}
            {hasOverrides ? <Badge variant="diamond" icon="tune">{t("instances.override")}</Badge> : null}
          </div>
        </div>
        <IconButton icon="folder_open" label={t("common.openFolder")} onClick={onOpen} />
        <IconButton icon="tune" label={t("instances.settings")} onClick={onEdit} />
      </div>

      {instance.description ? (
        <p className="text-sm leading-relaxed" style={{ color: "var(--text-body)" }}>
          {instance.description}
        </p>
      ) : null}

      <div className="divide-y" style={{ borderColor: "color-mix(in srgb, var(--border) 60%, transparent)" }}>
        <KeyValue label={t("instances.minecraft")} value={instance.minecraftVersion} mono />
        <KeyValue label={t("instances.loader")} value={loaderLabel(instance.loader.kind)} />
        {instance.loader.kind !== "none" && instance.loader.kind !== "mcp" ? <KeyValue label={t("instances.build")} value={instance.loader.version} mono /> : null}
        {instance.loader.mcpFile ? <KeyValue label="MCP" value={instance.loader.mcpFile} mono /> : null}
        <KeyValue
          label={t("instances.java")}
          value={
            java ? (
              <span>
                Java {java.major}{" "}
                <span className="text-[10px] font-normal" style={{ color: "var(--text-meta)" }}>
                  ({t(`instances.javaFrom.${java.source}`)})
                </span>
              </span>
            ) : javaError ? (
              "—"
            ) : (
              <Icon name="autorenew" size={14} spin />
            )
          }
        />
        <KeyValue label={t("instances.files")} value={instance.filesUrl ? <Icon name="check" size={16} style={{ color: "#34d399" }} /> : t("instances.filesNone")} />
        <KeyValue label={t("instances.verifyMode")} value={instance.verify ? t("common.yes") : t("common.no")} />
        {instance.ignored.length > 0 ? <KeyValue label={t("instances.ignored")} value={<span className="font-mono text-[11px]">{instance.ignored.join(", ")}</span>} /> : null}
        {instance.server?.host ? (
          <KeyValue label={t("home.server")} value={`${instance.server.host}${instance.server.port ? `:${instance.server.port}` : ""}`} mono />
        ) : null}
        {instance.whitelistActive ? <KeyValue label={t("instances.whitelisted")} value={`${instance.whitelist.length}`} /> : null}
      </div>

      {game.instanceId === instance.id ? <LaunchProgress /> : null}

      <div className="flex flex-wrap gap-2 pt-1">
        <Button variant="primary" size="lg" icon="play_arrow" disabled={busy || running || !hasAccount} onClick={onPlay}>
          {t("common.play")}
        </Button>
        <Button variant="secondary" size="lg" icon={installed ? "verified" : "download"} disabled={busy || running} onClick={onInstall}>
          {installed ? t("common.verify") : t("common.install")}
        </Button>
        {selectedId !== instance.id ? (
          <Button variant="ghost" size="lg" icon="check" onClick={onSelect}>
            {t("common.select")}
          </Button>
        ) : null}
      </div>
    </Card>
  );
}
