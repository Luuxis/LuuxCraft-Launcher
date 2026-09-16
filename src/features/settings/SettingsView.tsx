/**
 * Settings page. Every control writes to a local draft that is persisted
 * (debounced) through the backend, which validates and clamps the values.
 */
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

import { describeError, t } from "../../i18n";
import { formatBytes, formatMemory, formatPercent } from "../../lib/format";
import { ipc, toAppError } from "../../lib/ipc";
import type { JavaInstall, JavaRequirement, Settings } from "../../lib/types";
import { useActions, useAppState, useBrand, useInstances, useUpdaterConfigured } from "../../store/AppStore";
import { Button, IconButton } from "../../components/ui/Button";
import { Field, Input, OptionCard, Select, Slider, Textarea, Toggle } from "../../components/ui/forms";
import { Icon } from "../../components/ui/Icon";
import { ConfirmDialog } from "../../components/ui/Modal";
import { Badge, Card, IconChip, KeyValue, Notice, ProgressBar, SectionHeader } from "../../components/ui/primitives";

const SAVE_DELAY = 400;

function useSettingsEditor() {
  const { settings } = useAppState();
  const { saveSettings, toast } = useActions();
  const [draft, setDraft] = useState<Settings | null>(settings);
  const timer = useRef<number | undefined>(undefined);
  const pendingRef = useRef<Settings | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!pendingRef.current) setDraft(settings);
  }, [settings]);

  const flush = useCallback(async () => {
    const next = pendingRef.current;
    pendingRef.current = null;
    if (!next) return;
    setSaving(true);
    const stored = await saveSettings(next);
    setSaving(false);
    if (stored) {
      setDraft(stored);
      toast("success", t("toasts.settingsSaved"));
    }
  }, [saveSettings, toast]);

  const update = useCallback(
    (patch: Partial<Settings> | ((current: Settings) => Settings)) => {
      setDraft((current) => {
        if (!current) return current;
        const next = typeof patch === "function" ? patch(current) : { ...current, ...patch };
        pendingRef.current = next;
        return next;
      });
      window.clearTimeout(timer.current);
      timer.current = window.setTimeout(() => void flush(), SAVE_DELAY);
    },
    [flush],
  );

  useEffect(() => () => window.clearTimeout(timer.current), []);

  return { draft, update, saving };
}

function Section({ id, icon, title, children, tone }: { id: string; icon: string; title: string; tone?: "brand" | "diamond" | "gold" | "error" | "pink"; children: ReactNode }) {
  return (
    <Card static id={id} className="space-y-5 scroll-mt-4">
      <div className="flex items-center gap-3">
        <IconChip icon={icon} size="sm" tone={tone} />
        <h3 className="text-lg font-bold" style={{ color: "var(--text-primary)" }}>
          {title}
        </h3>
      </div>
      {children}
    </Card>
  );
}

export function SettingsView() {
  const { bootstrap, update: updateCheckResult, updateInstalling, updateProgress } = useAppState();
  const { resetSettings, openFolder, checkUpdate, installUpdate, openExternal } = useActions();
  const { selected } = useInstances();
  const { draft, update, saving } = useSettingsEditor();
  const brand = useBrand();
  const updaterConfigured = useUpdaterConfigured();
  const [confirmReset, setConfirmReset] = useState(false);
  const [gameRoot, setGameRoot] = useState(bootstrap?.paths.gameRoot ?? "");

  const [javas, setJavas] = useState<JavaInstall[] | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [probe, setProbe] = useState<JavaInstall | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);
  const [required, setRequired] = useState<JavaRequirement | null>(null);
  const [checking, setChecking] = useState(false);
  // The custom Java choice is local until the executable has been probed
  // successfully: only then is `java.mode = custom` persisted.
  const [customJavaUi, setCustomJavaUi] = useState(false);
  const [javaPathInput, setJavaPathInput] = useState("");

  useEffect(() => {
    if (draft?.java.mode === "custom") {
      setCustomJavaUi(true);
      setJavaPathInput((current) => current || draft.java.path || "");
    }
  }, [draft?.java.mode, draft?.java.path]);

  useEffect(() => {
    ipc.gameRoot().then(setGameRoot).catch(() => undefined);
  }, [draft?.installPath]);

  useEffect(() => {
    if (!selected) return setRequired(null);
    let cancelled = false;
    ipc
      .javaRequired(selected.id)
      .then((req) => !cancelled && setRequired(req))
      .catch(() => !cancelled && setRequired(null));
    return () => {
      cancelled = true;
    };
  }, [selected]);

  useEffect(() => {
    const path = customJavaUi ? javaPathInput.trim() : "";
    if (!path) {
      setProbe(null);
      setProbeError(null);
      return;
    }
    let cancelled = false;
    const handle = window.setTimeout(() => {
      ipc
        .javaProbe(path)
        .then((install) => {
          if (cancelled) return;
          setProbe(install);
          setProbeError(null);
          update((current) =>
            current.java.mode === "custom" && current.java.path === install.path ? current : { ...current, java: { mode: "custom", path: install.path } },
          );
        })
        .catch((cause) => {
          if (cancelled) return;
          setProbe(null);
          setProbeError(describeError(toAppError(cause)));
        });
    }, 350);
    return () => {
      cancelled = true;
      window.clearTimeout(handle);
    };
  }, [customJavaUi, javaPathInput, update]);

  if (!draft || !bootstrap) return null;

  const totalMb = bootstrap.system.totalMemoryMb;
  const memoryCeiling = Math.max(1024, (totalMb ?? 16384) - 1024);
  const platformIsAppleSilicon = bootstrap.system.platform === "macos" && bootstrap.system.arch === "aarch64";
  

  const detectJava = async () => {
    setDetecting(true);
    try {
      setJavas(await ipc.javaDetect());
    } catch {
      setJavas([]);
    } finally {
      setDetecting(false);
    }
  };

  const browseJava = async () => {
    const picked = await openDialog({ multiple: false, directory: false, title: t("settings.java.path") });
    if (typeof picked === "string") {
      setCustomJavaUi(true);
      setJavaPathInput(picked);
    }
  };

  const chooseAutoJava = () => {
    setCustomJavaUi(false);
    setProbe(null);
    setProbeError(null);
    update({ java: { mode: "auto", path: null } });
  };

  const useJava = (path: string) => {
    setCustomJavaUi(true);
    setJavaPathInput(path);
  };

  const browseInstall = async () => {
    const picked = await openDialog({ multiple: false, directory: true, title: t("settings.install.path"), defaultPath: gameRoot || undefined });
    if (typeof picked === "string") update({ installPath: picked });
  };

  const runUpdateCheck = async () => {
    setChecking(true);
    await checkUpdate();
    setChecking(false);
  };

  const javaIncompatible = probe && required && probe.major !== required.major;

  return (
    <div className="space-y-6 animate-fade-in-up">
      <SectionHeader
        icon="settings"
        title={t("settings.title")}
        description={t("settings.subtitle")}
        actions={
          <>
            {saving ? (
              <span className="text-xs inline-flex items-center gap-1.5" style={{ color: "var(--text-meta)" }}>
                <Icon name="autorenew" size={14} spin /> {t("common.saving")}
              </span>
            ) : null}
            <Button variant="ghost" icon="restart_alt" onClick={() => setConfirmReset(true)}>
              {t("common.reset")}
            </Button>
          </>
        }
      />

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-5 items-start">
        {/* Java */}
        <Section id="java" icon="coffee" title={t("settings.sections.java")}>
          <div className="space-y-2" role="radiogroup">
            <OptionCard checked={!customJavaUi} onSelect={chooseAutoJava} icon="auto_awesome" title={t("settings.java.auto")} description={t("settings.java.autoHint")} />
            <OptionCard checked={customJavaUi} onSelect={() => setCustomJavaUi(true)} icon="folder" title={t("settings.java.custom")} description={t("settings.java.customHint")} />
          </div>
          {required ? (
            <p className="text-xs flex items-center gap-1.5" style={{ color: "var(--text-meta)" }}>
              <Icon name="info" size={14} /> {t("settings.java.required", { major: required.major })} ({selected?.name})
            </p>
          ) : null}
          {customJavaUi ? (
            <div className="space-y-3">
              <Field label={t("settings.java.path")} icon="terminal">
                <div className="flex gap-2">
                  <Input mono value={javaPathInput} placeholder="/chemin/vers/bin/java" onChange={(e) => setJavaPathInput(e.target.value)} />
                  <Button variant="secondary" square icon="folder_open" onClick={() => void browseJava()}>
                    {t("common.browse")}
                  </Button>
                </div>
              </Field>
              {probe ? (
                <Notice tone={javaIncompatible ? "warning" : "success"} icon="coffee">
                  {t("settings.java.probeOk", { version: probe.version, vendor: probe.vendor, arch: probe.arch })}
                  {javaIncompatible ? ` ${t("settings.java.incompatible", { required: required?.major ?? "?" })}` : ""}
                </Notice>
              ) : probeError ? (
                <Notice tone="error">{probeError}</Notice>
              ) : null}
            </div>
          ) : null}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="field-label">
                <Icon name="search" size={18} /> {t("settings.java.detected")}
              </span>
              <Button variant="secondary" size="xs" icon="radar" loading={detecting} onClick={() => void detectJava()}>
                {t("settings.java.detect")}
              </Button>
            </div>
            {javas && javas.length === 0 ? (
              <p className="text-xs" style={{ color: "var(--text-meta)" }}>
                {t("settings.java.noneDetected")}
              </p>
            ) : null}
            {javas && javas.length > 0 ? (
              <ul className="space-y-2">
                {javas.map((java) => (
                  <li key={java.path} className="card-inset px-3 py-2.5 flex items-center gap-3">
                    <Badge variant={required && java.major === required.major ? "brand" : "neutral"}>Java {java.major}</Badge>
                    <div className="flex-1 min-w-0">
                      <p className="text-xs font-medium truncate" style={{ color: "var(--text-primary)" }}>
                        {java.version} · {java.vendor} · {java.arch}
                        {java.managed ? (
                          <span className="ml-2 text-[10px]" style={{ color: "#67e8f9" }}>
                            {t("settings.java.managed")}
                          </span>
                        ) : null}
                      </p>
                      <p className="text-[10px] font-mono truncate selectable" style={{ color: "var(--text-placeholder)" }}>
                        {java.path}
                      </p>
                    </div>
                    <Button variant="ghost" size="xs" onClick={() => useJava(java.path)} disabled={customJavaUi && javaPathInput === java.path}>
                      {t("settings.java.use")}
                    </Button>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        </Section>

        {/* Mémoire */}
        <Section id="memory" icon="memory" title={t("settings.sections.memory")} tone="diamond">
          <p className="text-xs" style={{ color: "var(--text-meta)" }}>
            {t("settings.memory.hint")}
            {totalMb ? ` ${t("settings.memory.total", { total: formatMemory(totalMb) })}.` : ""}
          </p>
          <Field label={t("settings.memory.min")} icon="memory">
            <Slider
              value={draft.memory.minMb}
              min={512}
              max={memoryCeiling}
              step={256}
              format={formatMemory}
              onChange={(v) => update((c) => ({ ...c, memory: { minMb: v, maxMb: Math.max(v, c.memory.maxMb) } }))}
            />
          </Field>
          <Field label={t("settings.memory.max")} icon="memory">
            <Slider
              value={draft.memory.maxMb}
              min={512}
              max={memoryCeiling}
              step={256}
              format={formatMemory}
              onChange={(v) => update((c) => ({ ...c, memory: { maxMb: v, minMb: Math.min(v, c.memory.minMb) } }))}
            />
          </Field>
          {totalMb && draft.memory.maxMb > totalMb * 0.75 ? <Notice tone="warning">{t("settings.memory.warning", { limit: formatMemory(Math.round(totalMb * 0.75)) })}</Notice> : null}
          <Field label={t("settings.memory.jvmArgs")} icon="code" hint={t("settings.memory.jvmArgsHint")}>
            <Textarea
              defaultValue={draft.jvmArgs.join("\n")}
              key={draft.jvmArgs.join("\n")}
              placeholder="-XX:+UseG1GC"
              onBlur={(e) => update({ jvmArgs: e.target.value.split("\n").map((s) => s.trim()).filter(Boolean) })}
            />
          </Field>
        </Section>

        {/* Fenêtre */}
        <Section id="window" icon="aspect_ratio" title={t("settings.sections.window")}>
          <p className="text-xs" style={{ color: "var(--text-meta)" }}>
            {t("settings.window.hint")}
          </p>
          <div className="grid grid-cols-2 gap-3">
            <Field label={t("settings.window.width")} icon="width">
              <Input
                type="number"
                min={320}
                max={7680}
                mono
                value={draft.gameWindow.width ?? ""}
                onChange={(e) => update({ gameWindow: { ...draft.gameWindow, width: e.target.value ? Number(e.target.value) : null } })}
              />
            </Field>
            <Field label={t("settings.window.height")} icon="height">
              <Input
                type="number"
                min={240}
                max={4320}
                mono
                value={draft.gameWindow.height ?? ""}
                onChange={(e) => update({ gameWindow: { ...draft.gameWindow, height: e.target.value ? Number(e.target.value) : null } })}
              />
            </Field>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs" style={{ color: "var(--text-meta)" }}>
              {t("settings.window.presets")}
            </span>
            {[
              [1280, 720],
              [1600, 900],
              [1920, 1080],
              [2560, 1440],
            ].map(([w, h]) => (
              <Button key={`${w}x${h}`} variant="ghost" size="xs" onClick={() => update({ gameWindow: { ...draft.gameWindow, width: w!, height: h! } })}>
                <span className="font-mono">
                  {w}×{h}
                </span>
              </Button>
            ))}
          </div>
          <Toggle checked={draft.gameWindow.fullscreen} onChange={(v) => update({ gameWindow: { ...draft.gameWindow, fullscreen: v } })} label={t("settings.window.fullscreen")} />
        </Section>

        {/* Téléchargements */}
        <Section id="downloads" icon="download" title={t("settings.sections.downloads")} tone="diamond">
          <Field label={t("settings.downloads.concurrency")} icon="swap_vert" hint={t("settings.downloads.hint", { max: bootstrap.config.downloads.maxConcurrency })}>
            <Slider value={draft.downloadConcurrency} min={1} max={bootstrap.config.downloads.maxConcurrency} onChange={(v) => update({ downloadConcurrency: v })} />
          </Field>
        </Section>

        {/* Comportement */}
        <Section id="behavior" icon="sports_esports" title={t("settings.sections.behavior")}>
          <div className="space-y-2" role="radiogroup">
            <OptionCard checked={draft.launcherBehavior === "stay"} onSelect={() => update({ launcherBehavior: "stay" })} icon="web_asset" title={t("settings.behavior.stay")} description={t("settings.behavior.stayHint")} />
            <OptionCard checked={draft.launcherBehavior === "hide"} onSelect={() => update({ launcherBehavior: "hide" })} icon="visibility_off" title={t("settings.behavior.hide")} description={t("settings.behavior.hideHint")} />
            <OptionCard checked={draft.launcherBehavior === "bound"} onSelect={() => update({ launcherBehavior: "bound" })} icon="link" title={t("settings.behavior.bound")} description={t("settings.behavior.boundHint")} />
          </div>
          {platformIsAppleSilicon ? (
            <Toggle checked={draft.intelEnabledMac} onChange={(v) => update({ intelEnabledMac: v })} label={t("settings.behavior.intelMac")} description={t("settings.behavior.intelMacHint")} />
          ) : null}
        </Section>

        {/* Installation */}
        <Section id="install" icon="folder" title={t("settings.sections.install")} tone="gold">
          <Field label={t("settings.install.path")} icon="folder" hint={t("settings.install.pathHint")}>
            <div className="flex gap-2">
              <Input mono readOnly value={gameRoot} />
              <IconButton icon="folder_open" label={t("common.openFolder")} onClick={() => void openFolder("gameRoot")} />
            </div>
          </Field>
          <div className="flex flex-wrap gap-2">
            <Button variant="secondary" icon="drive_file_move" onClick={() => void browseInstall()}>
              {t("settings.install.change")}
            </Button>
            {draft.installPath ? (
              <Button variant="ghost" icon="undo" onClick={() => update({ installPath: null })}>
                {t("settings.install.useDefault")}
              </Button>
            ) : null}
          </div>
          <Notice tone="warning">{t("settings.install.changeWarning")}</Notice>
        </Section>

        {/* Serveur : le tenant que le pack client a figé à l'installation */}
        <Section id="server" icon="dns" title={t("settings.sections.server")}>
          <KeyValue label={t("settings.server.name")} value={bootstrap.config.displayName} />
          <KeyValue label={t("settings.server.slug")} value={bootstrap.config.slug} mono />
          <KeyValue label={t("settings.server.tenant")} value={bootstrap.config.userId} mono />
          <KeyValue label={t("settings.server.panel")} value={bootstrap.config.api.baseUrl} mono />
          <Notice tone="info">{t("settings.server.hint")}</Notice>
        </Section>

        {/* Interface */}
        <Section id="interface" icon="palette" title={t("settings.sections.interface")} tone="pink">
          <Field label={t("settings.interface.theme")} icon="dark_mode">
            <Select<"dark" | "light" | "system">
              value={draft.ui.theme}
              onChange={(theme) => update({ ui: { ...draft.ui, theme } })}
              options={[
                { value: "dark", label: t("settings.interface.themes.dark"), icon: "dark_mode" },
                { value: "light", label: t("settings.interface.themes.light"), icon: "light_mode" },
                { value: "system", label: t("settings.interface.themes.system"), icon: "computer" },
              ]}
            />
          </Field>
          <Toggle checked={draft.ui.reduceMotion} onChange={(v) => update({ ui: { ...draft.ui, reduceMotion: v } })} label={t("settings.interface.reduceMotion")} />
          <Field label={t("settings.interface.statusRefresh")} icon="schedule">
            <Slider value={draft.serverStatusRefreshSeconds} min={10} max={300} step={5} format={(v) => t("settings.interface.seconds", { value: v })} onChange={(v) => update({ serverStatusRefreshSeconds: v })} />
          </Field>
        </Section>

        {/* Mises à jour */}
        <Section id="updates" icon="system_update" title={t("settings.sections.updates")}>
          {!updaterConfigured ? (
            <Notice tone="info">{t("settings.updates.notConfigured")}</Notice>
          ) : (
            <>
              <Toggle checked={draft.checkUpdatesOnStartup} onChange={(v) => update({ checkUpdatesOnStartup: v })} label={t("settings.updates.auto")} />
              <div className="flex items-center gap-3 flex-wrap">
                <Button variant="secondary" icon="refresh" loading={checking} onClick={() => void runUpdateCheck()}>
                  {t("settings.updates.check")}
                </Button>
                {updateCheckResult?.update ? (
                  <Button variant="primary" icon="download" loading={updateInstalling} onClick={() => void installUpdate()}>
                    {t("settings.updates.install")}
                  </Button>
                ) : null}
                {updateCheckResult && !updateCheckResult.update ? (
                  <span className="text-xs inline-flex items-center gap-1.5" style={{ color: "#6ee7b7" }}>
                    <Icon name="check_circle" size={14} /> {t("settings.updates.upToDate", { version: bootstrap.system.launcherVersion })}
                  </span>
                ) : null}
              </div>
              {updateCheckResult?.update ? (
                <div className="card-inset p-4 space-y-2">
                  <p className="text-sm font-semibold" style={{ color: "var(--text-primary)" }}>
                    {t("settings.updates.available", { version: updateCheckResult.update.version })}
                  </p>
                  {updateCheckResult.update.body ? (
                    <p className="text-xs whitespace-pre-wrap selectable" style={{ color: "var(--text-body)" }}>
                      {updateCheckResult.update.body}
                    </p>
                  ) : null}
                  {updateProgress ? (
                    <div className="space-y-1">
                      <ProgressBar value={updateProgress.total ? formatPercent(updateProgress.downloaded, updateProgress.total) : 0} indeterminate={!updateProgress.total} />
                      <p className="text-[11px] font-mono" style={{ color: "var(--text-meta)" }}>
                        {formatBytes(updateProgress.downloaded)}
                        {updateProgress.total ? ` / ${formatBytes(updateProgress.total)}` : ""}
                      </p>
                    </div>
                  ) : null}
                </div>
              ) : null}
            </>
          )}
        </Section>

        {/* Logs */}
        <Section id="logs" icon="bug_report" title={t("settings.sections.logs")} tone="error">
          <p className="text-xs" style={{ color: "var(--text-meta)" }}>
            {t("settings.logs.hint")}
          </p>
          <div className="flex flex-wrap gap-2">
            <Button variant="secondary" icon="folder_open" onClick={() => void openFolder("logs")}>
              {t("settings.logs.open")}
            </Button>
            <Button variant="ghost" icon="terminal" onClick={() => void openFolder("gameLogs")}>
              {t("settings.logs.openGame")}
            </Button>
            <Button variant="ghost" icon="database" onClick={() => void openFolder("launcherData")}>
              {t("settings.logs.openData")}
            </Button>
          </div>
        </Section>

        {/* À propos */}
        <Section id="about" icon="info" title={t("settings.sections.about")}>
          <div className="divide-y" style={{ borderColor: "color-mix(in srgb, var(--border) 60%, transparent)" }}>
            <KeyValue label={t("settings.about.version")} value={bootstrap.system.launcherVersion} mono />
            <KeyValue label={t("settings.about.engine")} value="crust_core 1.0.3" mono />
            <KeyValue label={t("settings.about.platform")} value={`${bootstrap.system.platform} · ${bootstrap.system.arch}`} mono />
            <KeyValue label={t("settings.about.storage")} value={<span className="selectable">{bootstrap.system.accountsFile}</span>} mono />
          </div>
          {brand.website ? (
            <Button variant="ghost" size="xs" icon="open_in_new" onClick={() => void openExternal(brand.website!)}>
              {brand.website}
            </Button>
          ) : null}
        </Section>
      </div>

      <ConfirmDialog
        open={confirmReset}
        title={t("common.reset")}
        message={t("settings.resetConfirm")}
        confirmLabel={t("common.reset")}
        danger
        onCancel={() => setConfirmReset(false)}
        onConfirm={async () => {
          await resetSettings();
          setConfirmReset(false);
        }}
      />
    </div>
  );
}
