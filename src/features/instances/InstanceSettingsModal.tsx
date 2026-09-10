import { useEffect, useState } from "react";

import { t } from "../../i18n";
import { formatMemory } from "../../lib/format";
import type { Instance, InstanceSettings, JavaInstall } from "../../lib/types";
import { ipc } from "../../lib/ipc";
import { useActions, useAppState } from "../../store/AppStore";
import { Button } from "../../components/ui/Button";
import { Field, Input, Select, Slider, Textarea, Toggle } from "../../components/ui/forms";
import { Modal } from "../../components/ui/Modal";
import { Notice } from "../../components/ui/primitives";

interface Props {
  instance: Instance | null;
  onClose: () => void;
}

/** Per-instance overrides of memory, Java and JVM arguments. */
export function InstanceSettingsModal({ instance, onClose }: Props) {
  const { settings, bootstrap } = useAppState();
  const { saveSettings } = useActions();
  const [draft, setDraft] = useState<InstanceSettings>({ memory: null, java: null, jvmArgs: null });
  const [javas, setJavas] = useState<JavaInstall[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!instance || !settings) return;
    setDraft(settings.instances[instance.id] ?? { memory: null, java: null, jvmArgs: null });
    ipc.javaDetect().then(setJavas).catch(() => setJavas([]));
  }, [instance, settings]);

  if (!instance || !settings) return null;
  const totalMb = bootstrap?.system.totalMemoryMb ?? 16384;
  const maxMb = Math.max(1024, totalMb - 1024);
  const memory = draft.memory ?? settings.memory;
  const java = draft.java ?? settings.java;

  const save = async () => {
    setSaving(true);
    await saveSettings((current) => ({ ...current, instances: { ...current.instances, [instance.id]: draft } }));
    setSaving(false);
    onClose();
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={t("instances.settings")}
      subtitle={instance.name}
      icon="tune"
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button variant="primary" icon="save" loading={saving} onClick={() => void save()}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <p className="text-sm" style={{ color: "var(--text-body)" }}>
        {t("instances.settingsHint")}
      </p>
      {instance.memory && (instance.memory.minMb || instance.memory.maxMb) ? (
        <Notice tone="info">
          {t("instances.memoryRecommended", {
            min: formatMemory(instance.memory.minMb ?? settings.memory.minMb),
            max: formatMemory(instance.memory.maxMb ?? settings.memory.maxMb),
          })}
        </Notice>
      ) : null}

      <div className="space-y-3">
        <Toggle
          label={t("settings.sections.memory")}
          description={draft.memory ? t("instances.override") : t("instances.useGlobal")}
          checked={draft.memory !== null}
          onChange={(on) => setDraft((d) => ({ ...d, memory: on ? { ...settings.memory } : null }))}
        />
        {draft.memory ? (
          <div className="space-y-4 pl-1">
            <Field label={t("settings.memory.min")} icon="memory">
              <Slider
                value={memory.minMb}
                min={512}
                max={maxMb}
                step={256}
                format={formatMemory}
                onChange={(v) => setDraft((d) => ({ ...d, memory: { minMb: v, maxMb: Math.max(v, d.memory?.maxMb ?? v) } }))}
              />
            </Field>
            <Field label={t("settings.memory.max")} icon="memory">
              <Slider
                value={memory.maxMb}
                min={512}
                max={maxMb}
                step={256}
                format={formatMemory}
                onChange={(v) => setDraft((d) => ({ ...d, memory: { maxMb: v, minMb: Math.min(v, d.memory?.minMb ?? v) } }))}
              />
            </Field>
          </div>
        ) : null}
      </div>

      <div className="space-y-3">
        <Toggle
          label={t("settings.sections.java")}
          description={draft.java ? t("instances.override") : t("instances.useGlobal")}
          checked={draft.java !== null}
          onChange={(on) => setDraft((d) => ({ ...d, java: on ? { ...settings.java } : null }))}
        />
        {draft.java ? (
          <div className="space-y-3 pl-1">
            <Select<"auto" | "custom">
              value={java.mode}
              onChange={(mode) => setDraft((d) => ({ ...d, java: { mode, path: mode === "auto" ? null : (d.java?.path ?? javas[0]?.path ?? null) } }))}
              options={[
                { value: "auto", label: t("settings.java.auto"), icon: "auto_awesome" },
                { value: "custom", label: t("settings.java.custom"), icon: "folder" },
              ]}
            />
            {java.mode === "custom" ? (
              <>
                {javas.length > 0 ? (
                  <Select<string>
                    value={java.path}
                    placeholder={t("settings.java.detected")}
                    onChange={(path) => setDraft((d) => ({ ...d, java: { mode: "custom", path } }))}
                    options={javas.map((j) => ({ value: j.path, label: `Java ${j.major} · ${j.vendor}`, description: j.path, icon: "coffee" }))}
                  />
                ) : null}
                <Input mono value={java.path ?? ""} placeholder="/chemin/vers/bin/java" onChange={(e) => setDraft((d) => ({ ...d, java: { mode: "custom", path: e.target.value } }))} />
              </>
            ) : null}
          </div>
        ) : null}
      </div>

      <div className="space-y-3">
        <Toggle
          label={t("settings.memory.jvmArgs")}
          description={draft.jvmArgs ? t("instances.override") : t("instances.useGlobal")}
          checked={draft.jvmArgs !== null}
          onChange={(on) => setDraft((d) => ({ ...d, jvmArgs: on ? [...settings.jvmArgs] : null }))}
        />
        {draft.jvmArgs ? (
          <Textarea
            value={draft.jvmArgs.join("\n")}
            placeholder="-XX:+UseG1GC"
            onChange={(e) => setDraft((d) => ({ ...d, jvmArgs: e.target.value.split("\n").map((s) => s.trim()).filter(Boolean) }))}
          />
        ) : null}
      </div>
    </Modal>
  );
}
