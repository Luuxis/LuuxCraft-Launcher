import { useCallback, useEffect, useState } from "react";

import { describeError, t } from "../../i18n";
import { ipc, toAppError } from "../../lib/ipc";
import type { AppError, SkinData } from "../../lib/types";
import { useActions, useSelectedAccount } from "../../store/AppStore";
import { Button, IconButton } from "../../components/ui/Button";
import { Field, Select, Slider, Toggle } from "../../components/ui/forms";
import { Badge, Card, EmptyState, Notice, SectionHeader, Skeleton } from "../../components/ui/primitives";
import { SkinViewer3D, type AnimationName, type ModelName } from "./SkinViewer3D";

export function SkinsView() {
  const account = useSelectedAccount();
  const { openLogin, toast } = useActions();
  const [data, setData] = useState<SkinData | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  const [animation, setAnimation] = useState<AnimationName>("walk");
  const [model, setModel] = useState<ModelName>("auto");
  const [autoRotate, setAutoRotate] = useState(false);
  const [zoom, setZoom] = useState(0.65);
  const [elytra, setElytra] = useState(false);
  const [resetToken, setResetToken] = useState(0);

  const load = useCallback(
    async (refresh: boolean) => {
      if (!account) {
        setData(null);
        return;
      }
      setLoading(true);
      setError(null);
      try {
        const skin = await ipc.skinGet(account.uuid, refresh);
        setData(skin);
        setModel(skin.model);
        if (refresh) toast("success", t("common.refresh"), account.name);
      } catch (cause) {
        setError(toAppError(cause));
      } finally {
        setLoading(false);
      }
    },
    [account, toast],
  );

  useEffect(() => {
    void load(false);
  }, [load]);

  return (
    <div className="space-y-6 animate-fade-in-up">
      <SectionHeader
        icon="person"
        title={t("skins.title")}
        description={t("skins.subtitle")}
        actions={
          account ? (
            <Button variant="secondary" icon="sync" loading={loading} onClick={() => void load(true)} disabled={account.kind === "offline"}>
              {t("skins.refresh")}
            </Button>
          ) : null
        }
      />

      {!account ? (
        <Card static>
          <EmptyState
            icon="person"
            title={t("skins.noAccount")}
            action={
              <Button variant="primary" square icon="person_add" onClick={() => openLogin(true)}>
                {t("accounts.add")}
              </Button>
            }
          />
        </Card>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-[minmax(0,1fr)_320px] gap-5">
          <Card premium static padding="p-0" className="min-h-[520px] flex flex-col">
            <div className="flex items-center justify-between px-5 py-3" style={{ borderBottom: "1px solid color-mix(in srgb, var(--border) 60%, transparent)" }}>
              <div className="flex items-center gap-2 min-w-0">
                <span className="text-sm font-bold truncate" style={{ color: "var(--text-primary)" }}>
                  {account.name}
                </span>
                <Badge variant="neutral">{t(`accounts.kinds.${account.kind}`)}</Badge>
                {data?.model && data.model !== "auto" ? <Badge variant="diamond">{t(`skins.models.${data.model}`)}</Badge> : null}
                {data?.cape ? (
                  <Badge variant="gold" icon="checkroom">
                    {data.capeAlias ?? t("skins.cape")}
                  </Badge>
                ) : null}
              </div>
              <IconButton icon="center_focus_strong" label={t("skins.resetCamera")} onClick={() => setResetToken((n) => n + 1)} />
            </div>
            <div className="relative flex-1 min-h-[440px]">
              <div
                className="absolute inset-0 pointer-events-none"
                style={{
                  background:
                    "radial-gradient(ellipse 60% 50% at 50% 60%, rgba(34,197,94,0.14), rgba(34,211,238,0.05) 40%, transparent 70%)",
                }}
              />
              {loading && !data ? (
                <div className="absolute inset-0 flex items-center justify-center">
                  <Skeleton className="w-40 h-72" />
                </div>
              ) : (
                <SkinViewer3D
                  skin={data?.skin ?? null}
                  cape={data?.cape ?? null}
                  model={model}
                  animation={animation}
                  autoRotate={autoRotate}
                  zoom={zoom}
                  elytra={elytra}
                  resetToken={resetToken}
                />
              )}
            </div>
            <p className="px-5 py-2.5 text-[11px]" style={{ color: "var(--text-meta)", borderTop: "1px solid color-mix(in srgb, var(--border) 60%, transparent)" }}>
              {t("skins.hint")}
            </p>
          </Card>

          <div className="space-y-4">
            {error ? <Notice tone="error" details={error.details}>{describeError(error)}</Notice> : null}
            {data && !data.skin ? <Notice tone="info">{t("skins.defaultSkin")}</Notice> : null}
            <Card static className="space-y-5">
              <Field label={t("skins.animation")} icon="directions_walk">
                <Select<AnimationName>
                  value={animation}
                  onChange={setAnimation}
                  options={(["idle", "walk", "run", "wave", "fly", "none"] as AnimationName[]).map((name) => ({
                    value: name,
                    label: t(`skins.animations.${name}`),
                  }))}
                />
              </Field>
              <Field label={t("skins.model")} icon="accessibility_new">
                <Select<ModelName>
                  value={model}
                  onChange={setModel}
                  options={(["auto", "default", "slim"] as ModelName[]).map((name) => ({ value: name, label: t(`skins.models.${name}`) }))}
                />
              </Field>
              <Field label={t("skins.zoom")} icon="zoom_in">
                <Slider value={Math.round(zoom * 100)} min={40} max={200} onChange={(v) => setZoom(v / 100)} format={(v) => `${v} %`} />
              </Field>
              <Toggle checked={autoRotate} onChange={setAutoRotate} label={t("skins.autoRotate")} />
              <Toggle checked={elytra} onChange={setElytra} label={t("skins.capeElytra")} disabled={!data?.cape} description={data?.cape ? undefined : t("skins.capeNone")} />
            </Card>
            {account.kind === "microsoft" ? (
              <p className="text-xs leading-relaxed px-1" style={{ color: "var(--text-meta)" }}>
                {t("skins.changeHint")}
              </p>
            ) : null}
          </div>
        </div>
      )}
    </div>
  );
}
