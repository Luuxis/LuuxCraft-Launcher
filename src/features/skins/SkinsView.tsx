import { useCallback, useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { describeError, t } from "../../i18n";
import { ipc, toAppError, type SkinDraft } from "../../lib/ipc";
import type { AppError, CapeOption, LibrarySkin, SkinData } from "../../lib/types";
import { useActions, useSelectedAccount } from "../../store/AppStore";
import { Button, IconButton } from "../../components/ui/Button";
import { Icon } from "../../components/ui/Icon";
import { ConfirmDialog } from "../../components/ui/Modal";
import { Badge, Card, EmptyState, Notice, SectionHeader, Skeleton } from "../../components/ui/primitives";
import { SkinBody } from "./SkinBody";
import { SkinEditor, type EditorTarget } from "./SkinEditor";
import { SkinViewer3D, type ModelName } from "./SkinViewer3D";

type Busy = "apply" | "reset" | "import" | "save" | null;

/** The camera framing of the preview; the wheel zooms from there. */
const VIEWER_ZOOM = 0.7;

export function SkinsView() {
  const account = useSelectedAccount();
  const { openLogin, toast, toastError, reloadAccounts } = useActions();

  const [data, setData] = useState<SkinData | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  const [library, setLibrary] = useState<LibrarySkin[]>([]);
  const [libraryLoading, setLibraryLoading] = useState(true);
  const [capes, setCapes] = useState<CapeOption[]>([]);
  /** The library skin being tried on: shown in the viewer, not applied yet. */
  const [preview, setPreview] = useState<LibrarySkin | null>(null);
  const [busy, setBusy] = useState<Busy>(null);
  const [editor, setEditor] = useState<EditorTarget | null>(null);
  const [removing, setRemoving] = useState<LibrarySkin | null>(null);
  const [resetting, setResetting] = useState(false);

  const [autoRotate, setAutoRotate] = useState(false);
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

  // Changing account drops a preview that no longer means anything.
  useEffect(() => setPreview(null), [account?.uuid]);

  useEffect(() => {
    if (!account) {
      setCapes([]);
      return;
    }
    let cancelled = false;
    ipc
      .skinCapesList(account.uuid)
      .then((owned) => !cancelled && setCapes(owned))
      .catch(() => !cancelled && setCapes([]));
    return () => {
      cancelled = true;
    };
  }, [account?.uuid, data?.cape]);

  const loadLibrary = useCallback(async () => {
    try {
      setLibrary(await ipc.skinLibraryList());
    } catch (cause) {
      toastError(cause);
    } finally {
      setLibraryLoading(false);
    }
  }, [toastError]);

  useEffect(() => {
    void loadLibrary();
  }, [loadLibrary]);

  // Dropping PNG files anywhere on the window imports them straight away.
  const importDropped = useCallback(
    async (paths: string[]) => {
      setBusy("import");
      let added: LibrarySkin | null = null;
      for (const path of paths) {
        try {
          added = await ipc.skinLibraryImport(path, { name: "", variant: "classic", capeId: null });
        } catch (cause) {
          toastError(cause);
        }
      }
      setBusy(null);
      if (!added) return;
      await loadLibrary();
      setPreview(added);
      toast("success", t("skins.imported", { name: added.name }));
    },
    [loadLibrary, toast, toastError],
  );

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type !== "drop") return;
        const pngs = event.payload.paths.filter((path) => path.toLowerCase().endsWith(".png"));
        if (pngs.length > 0) void importDropped(pngs);
      })
      .then((stop) => {
        if (cancelled) stop();
        else unlisten = stop;
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [importDropped]);

  const addCurrentToLibrary = async () => {
    if (!account) return;
    setBusy("save");
    try {
      const added = await ipc.skinLibraryAddCurrent(account.uuid);
      await loadLibrary();
      toast("success", t("skins.addedToLibrary", { name: added.name }));
    } catch (cause) {
      toastError(cause);
    } finally {
      setBusy(null);
    }
  };

  const applySkin = useCallback(
    async (skin: LibrarySkin) => {
      if (!account) return;
      setBusy("apply");
      try {
        const fresh = await ipc.skinApply(account.uuid, skin.id);
        setData(fresh);
        setPreview(null);
        await reloadAccounts();
        toast("success", t("skins.applied", { name: skin.name }));
      } catch (cause) {
        toastError(cause);
      } finally {
        setBusy(null);
      }
    },
    [account, reloadAccounts, toast, toastError],
  );

  const resetSkin = async () => {
    if (!account) return;
    setBusy("reset");
    try {
      const fresh = await ipc.skinReset(account.uuid);
      setData(fresh);
      setPreview(null);
      await reloadAccounts();
      toast("success", t("skins.resetDone"));
    } catch (cause) {
      toastError(cause);
    } finally {
      setBusy(null);
      setResetting(false);
    }
  };

  /** Creating and editing both land here: `skin` is null when creating. */
  const saveDraft = useCallback(
    async (draft: SkinDraft, path: string | null, skin: LibrarySkin | null) => {
      // Creating always comes with a file: the editor cannot save without one.
      if (!skin && !path) return null;
      try {
        const stored = skin
          ? await ipc.skinLibraryUpdate(skin.id, draft, path)
          : await ipc.skinLibraryImport(path as string, draft);
        setLibrary((current) =>
          skin ? current.map((entry) => (entry.id === stored.id ? stored : entry)) : [stored, ...current],
        );
        setPreview((current) => (current?.id === stored.id ? stored : current));
        return stored;
      } catch (cause) {
        toastError(cause);
        return null;
      }
    },
    [toastError],
  );

  const removeSkin = async (skin: LibrarySkin) => {
    try {
      await ipc.skinLibraryRemove(skin.id);
      setLibrary((current) => current.filter((entry) => entry.id !== skin.id));
      setPreview((current) => (current?.id === skin.id ? null : current));
      toast("success", t("skins.removed"));
    } catch (cause) {
      toastError(cause);
    } finally {
      setRemoving(null);
    }
  };

  const canChange = data?.canChange ?? false;
  const working = busy !== null;
  const shownTexture = preview?.texture ?? data?.skin ?? null;
  const shownModel: ModelName = preview ? (preview.variant === "slim" ? "slim" : "default") : (data?.model ?? "auto");
  const shownCape = preview
    ? (capes.find((cape) => cape.id === preview.capeId)?.texture ?? null)
    : (data?.cape ?? null);

  return (
    <div className="h-full min-h-0 flex flex-col animate-fade-in-up">
      <div className="shrink-0">
        <SectionHeader
          icon="person"
          title={t("skins.title")}
          description={t("skins.subtitle")}
          actions={
            account ? (
              <Button
                variant="secondary"
                icon="sync"
                loading={loading}
                onClick={() => void load(true)}
                disabled={account.kind === "offline" || working}
              >
                {t("skins.refresh")}
              </Button>
            ) : null
          }
        />
      </div>

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
        <div className="flex-1 min-h-0 grid grid-cols-1 lg:grid-cols-[320px_minmax(0,1fr)] gap-5">
          {/* ── Actuel ─────────────────────────────────────────────── */}
          <Card premium static padding="p-0" className="flex flex-col min-h-0">
            <div
              className="flex items-center justify-between gap-2 px-5 py-3 shrink-0"
              style={{ borderBottom: "1px solid color-mix(in srgb, var(--border) 60%, transparent)" }}
            >
              <div className="flex items-center gap-2 min-w-0">
                <h3 className="text-sm font-bold" style={{ color: "var(--text-primary)" }}>
                  {t("skins.current")}
                </h3>
                {preview ? <Badge variant="diamond" icon="visibility">{t("skins.previewing")}</Badge> : null}
              </div>
              <div className="flex items-center gap-1.5 shrink-0">
                <IconButton
                  icon="3d_rotation"
                  label={t("skins.autoRotate")}
                  size={16}
                  aria-pressed={autoRotate}
                  style={
                    autoRotate
                      ? { borderColor: "rgba(34,197,94,0.45)", background: "rgba(34,197,94,0.12)", color: "#6ee7b7" }
                      : undefined
                  }
                  onClick={() => setAutoRotate((on) => !on)}
                />
                <IconButton icon="center_focus_strong" label={t("skins.resetCamera")} size={16} onClick={() => setResetToken((n) => n + 1)} />
              </div>
            </div>

            <div className="relative flex-1 min-h-0">
              <div
                className="absolute inset-0 pointer-events-none"
                style={{
                  background:
                    "radial-gradient(ellipse 60% 50% at 50% 60%, rgba(34,197,94,0.14), rgba(34,211,238,0.05) 40%, transparent 70%)",
                }}
              />
              {loading && !data ? (
                <div className="absolute inset-0 flex items-center justify-center">
                  <Skeleton className="w-32 h-56" />
                </div>
              ) : (
                <SkinViewer3D
                  skin={shownTexture}
                  cape={shownCape}
                  model={shownModel}
                  autoRotate={autoRotate}
                  zoom={VIEWER_ZOOM}
                  resetToken={resetToken}
                  onUserControl={() => setAutoRotate(false)}
                />
              )}
              {/* Out of the flow: the hint must not eat the viewer's height. */}
              <p
                className="absolute inset-x-3 bottom-2 text-[10px] text-center leading-snug pointer-events-none"
                style={{ color: "var(--text-placeholder)" }}
              >
                {t("skins.hint")}
              </p>
            </div>

            <div
              className="px-5 py-4 space-y-3 shrink-0"
              style={{ borderTop: "1px solid color-mix(in srgb, var(--border) 60%, transparent)" }}
            >
              <div className="flex flex-wrap items-center gap-1.5">
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

              {preview ? (
                <div className="flex flex-wrap gap-2">
                  <Button
                    variant="primary"
                    icon="check"
                    square
                    size="sm"
                    loading={busy === "apply"}
                    disabled={!canChange || working}
                    onClick={() => void applySkin(preview)}
                  >
                    {t("skins.apply")}
                  </Button>
                  <Button variant="ghost" icon="close" square size="sm" disabled={working} onClick={() => setPreview(null)}>
                    {t("skins.cancelPreview")}
                  </Button>
                </div>
              ) : (
                <div className="flex flex-wrap gap-2">
                  <Button
                    variant="secondary"
                    icon="library_add"
                    square
                    size="sm"
                    loading={busy === "save"}
                    disabled={!data?.skin || working}
                    onClick={() => void addCurrentToLibrary()}
                  >
                    {t("skins.addToLibrary")}
                  </Button>
                  {canChange ? (
                    <Button
                      variant="ghost"
                      icon="restart_alt"
                      square
                      size="sm"
                      loading={busy === "reset"}
                      disabled={working}
                      onClick={() => setResetting(true)}
                    >
                      {t("skins.reset")}
                    </Button>
                  ) : null}
                </div>
              )}
            </div>
          </Card>

          {/* ── Bibliothèque ───────────────────────────────────────── */}
          <div className="flex flex-col gap-4 min-h-0">
            {error ? <Notice tone="error" details={error.details}>{describeError(error)}</Notice> : null}
            {data && !data.skin ? <Notice tone="info">{t("skins.defaultSkin")}</Notice> : null}
            {data && !canChange ? <Notice tone="warning" icon="info">{t("skins.notSupported")}</Notice> : null}

            <Card premium static padding="p-0" className="flex flex-col min-h-0 flex-1">
              <div
                className="flex items-center justify-between gap-3 px-5 py-3 shrink-0"
                style={{ borderBottom: "1px solid color-mix(in srgb, var(--border) 60%, transparent)" }}
              >
                <div className="flex items-center gap-2 min-w-0">
                  <h3 className="text-sm font-bold" style={{ color: "var(--text-primary)" }}>
                    {t("skins.library")}
                  </h3>
                  {library.length > 0 ? <Badge variant="neutral">{library.length}</Badge> : null}
                </div>
                <Button variant="ghost" size="xs" icon="add" disabled={working} onClick={() => setEditor({ skin: null })}>
                  {t("skins.newSkin")}
                </Button>
              </div>

              {/* The only scrolling area of the page. */}
              <div className="flex-1 min-h-0 overflow-y-auto custom-scrollbar p-4">
                {libraryLoading ? (
                  <div className="grid grid-cols-2 sm:grid-cols-3 2xl:grid-cols-4 gap-3">
                    <Skeleton className="h-44" />
                    <Skeleton className="h-44" />
                    <Skeleton className="h-44" />
                  </div>
                ) : (
                  <div className="grid grid-cols-2 sm:grid-cols-3 2xl:grid-cols-4 gap-3">
                    <NewSkinTile busy={busy === "import"} onClick={() => setEditor({ skin: null })} />
                    {library.map((skin, index) => (
                      <SkinTile
                        key={skin.id}
                        skin={skin}
                        index={index}
                        capeAlias={capes.find((cape) => cape.id === skin.capeId)?.alias ?? null}
                        active={preview?.id === skin.id}
                        disabled={working}
                        onSelect={() => setPreview(preview?.id === skin.id ? null : skin)}
                        onApply={canChange ? () => void applySkin(skin) : undefined}
                        onEdit={() => setEditor({ skin })}
                        onRemove={() => setRemoving(skin)}
                      />
                    ))}
                  </div>
                )}
              </div>
            </Card>
          </div>
        </div>
      )}

      <SkinEditor
        target={editor}
        capes={capes}
        canApply={canChange}
        onClose={() => setEditor(null)}
        onSave={saveDraft}
        onApply={applySkin}
      />

      <ConfirmDialog
        open={removing !== null}
        title={t("skins.remove")}
        message={t("skins.removeConfirm", { name: removing?.name ?? "" })}
        confirmLabel={t("common.delete")}
        danger
        onCancel={() => setRemoving(null)}
        onConfirm={() => removing && void removeSkin(removing)}
      />

      <ConfirmDialog
        open={resetting}
        title={t("skins.reset")}
        message={t("skins.resetConfirm", { name: account?.name ?? "" })}
        confirmLabel={t("skins.reset")}
        loading={busy === "reset"}
        onCancel={() => setResetting(false)}
        onConfirm={() => void resetSkin()}
      />
    </div>
  );
}

// ── Tiles ───────────────────────────────────────────────────────────────────

function NewSkinTile({ busy, onClick }: { busy: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      className="flex flex-col items-center justify-center gap-3 rounded-xl px-3 py-6 h-full min-h-44 transition-all duration-200"
      style={{
        border: "1px dashed var(--border-strong)",
        background: "color-mix(in srgb, var(--bg-deep) 50%, transparent)",
      }}
      onClick={onClick}
      disabled={busy}
    >
      <span className="icon-chip w-12 h-12" style={{ borderRadius: 9999 }}>
        <Icon name={busy ? "autorenew" : "add"} size={24} spin={busy} />
      </span>
      <span className="text-xs font-semibold text-center" style={{ color: "var(--text-label)" }}>
        {busy ? t("skins.importing") : t("skins.newSkin")}
      </span>
      <span className="text-[10px] text-center leading-snug" style={{ color: "var(--text-meta)" }}>
        {t("skins.newSkinHint")}
      </span>
    </button>
  );
}

const COMPACT_ICON = { width: 28, height: 28, borderRadius: 8 };

interface TileProps {
  skin: LibrarySkin;
  index: number;
  capeAlias: string | null;
  active: boolean;
  disabled: boolean;
  onSelect: () => void;
  onApply?: () => void;
  onEdit: () => void;
  onRemove: () => void;
}

function SkinTile({ skin, index, capeAlias, active, disabled, onSelect, onApply, onEdit, onRemove }: TileProps) {
  return (
    <div className={`relative group animate-fade-in-up stagger-${Math.min(index + 1, 5)}`} style={{ opacity: 0 }}>
      <button
        type="button"
        className="w-full h-full flex flex-col items-center gap-2 rounded-xl px-3 py-3 transition-all duration-200"
        style={{
          background: active ? "rgba(34,197,94,0.08)" : "color-mix(in srgb, var(--bg-deep) 60%, transparent)",
          border: `1px solid ${active ? "rgba(34,197,94,0.45)" : "color-mix(in srgb, var(--border) 60%, transparent)"}`,
          boxShadow: active ? "var(--glow-sm)" : undefined,
        }}
        onClick={onSelect}
        disabled={disabled}
        aria-pressed={active}
      >
        <span className="text-xs font-semibold truncate max-w-full" style={{ color: "var(--text-primary)" }}>
          {skin.name}
        </span>
        <SkinBody texture={skin.texture} variant={skin.variant} height={116} title={skin.name} />
        <span className="flex items-center gap-1 flex-wrap justify-center">
          <span className="badge badge-neutral">{t(`skins.variants.${skin.variant}`)}</span>
          {capeAlias ? (
            <span className="badge badge-gold" title={capeAlias}>
              <Icon name="checkroom" size={11} />
            </span>
          ) : null}
        </span>
      </button>

      {/* Hover actions, out of the main button so they stay clickable. */}
      <div className="absolute top-2 right-2 flex gap-1 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity">
        {/* `.icon-button` fixes the 36px box in plain CSS, so the compact size
            goes through the style attribute rather than a utility class. */}
        <IconButton icon="edit" label={t("skins.edit")} size={14} style={COMPACT_ICON} disabled={disabled} onClick={onEdit} />
        <IconButton icon="delete" label={t("skins.remove")} size={14} style={COMPACT_ICON} danger disabled={disabled} onClick={onRemove} />
      </div>
      {onApply ? (
        <div className="absolute inset-x-2 bottom-2 opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity">
          <Button variant="primary" size="xs" icon="check" square className="w-full" disabled={disabled} onClick={onApply}>
            {t("skins.apply")}
          </Button>
        </div>
      ) : null}
    </div>
  );
}
