/**
 * Skin editor: name, player model, skin file and cape, with a live 3D preview.
 * Creating and editing share it — creating simply starts from an empty draft
 * and requires a file before it can be saved.
 */
import { useEffect, useMemo, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { t } from "../../i18n";
import { ipc, type SkinDraft } from "../../lib/ipc";
import type { CapeOption, LibrarySkin, SkinVariant } from "../../lib/types";
import { useActions } from "../../store/AppStore";
import { Button } from "../../components/ui/Button";
import { Field, Input } from "../../components/ui/forms";
import { Icon } from "../../components/ui/Icon";
import { Modal } from "../../components/ui/Modal";
import { CapeImage } from "./CapeImage";
import { SkinViewer3D } from "./SkinViewer3D";

export interface EditorTarget {
  /** The library entry being edited, or `null` to create a new one. */
  skin: LibrarySkin | null;
}

interface SkinEditorProps {
  target: EditorTarget | null;
  capes: CapeOption[];
  /** Whether the account can actually wear the result. */
  canApply: boolean;
  onClose: () => void;
  /** Saves the draft and returns the stored entry. */
  onSave: (draft: SkinDraft, path: string | null, skin: LibrarySkin | null) => Promise<LibrarySkin | null>;
  onApply: (skin: LibrarySkin) => Promise<void>;
}

export function SkinEditor({ target, capes, canApply, onClose, onSave, onApply }: SkinEditorProps) {
  const { toastError } = useActions();
  const existing = target?.skin ?? null;
  const creating = target !== null && existing === null;

  const [name, setName] = useState("");
  const [variant, setVariant] = useState<SkinVariant>("classic");
  const [capeId, setCapeId] = useState<string | null>(null);
  /** A file picked but not saved yet, previewed from its data URL. */
  const [pickedPath, setPickedPath] = useState<string | null>(null);
  const [pickedTexture, setPickedTexture] = useState<string | null>(null);
  const [saving, setSaving] = useState<null | "save" | "use">(null);

  // Reset the form every time the editor opens on another target.
  useEffect(() => {
    if (!target) return;
    setName(existing?.name ?? "");
    setVariant(existing?.variant ?? "classic");
    setCapeId(existing?.capeId ?? null);
    setPickedPath(null);
    setPickedTexture(null);
    setSaving(null);
  }, [target, existing]);

  const texture = pickedTexture ?? existing?.texture ?? null;
  const capeTexture = useMemo(
    () => capes.find((cape) => cape.id === capeId)?.texture ?? null,
    [capes, capeId],
  );
  const busy = saving !== null;
  const ready = Boolean(texture) && name.trim().length > 0;

  if (!target) return null;

  const browse = async () => {
    const picked = await openDialog({
      multiple: false,
      filters: [{ name: t("skins.fileFilter"), extensions: ["png"] }],
    }).catch(() => null);
    const path = Array.isArray(picked) ? picked[0] : picked;
    if (!path) return;
    try {
      const preview = await ipc.skinFilePreview(path);
      setPickedTexture(preview);
      setPickedPath(path);
      // A brand new skin takes the file name until the user types one.
      if (!name.trim()) setName(fileStem(path));
    } catch (cause) {
      toastError(cause);
    }
  };

  const save = async (andUse: boolean) => {
    setSaving(andUse ? "use" : "save");
    const stored = await onSave({ name: name.trim(), variant, capeId }, pickedPath, existing);
    if (stored && andUse) await onApply(stored);
    setSaving(null);
    if (stored) onClose();
  };

  return (
    <Modal
      open
      onClose={onClose}
      title={creating ? t("skins.newSkin") : t("skins.edit")}
      subtitle={creating ? t("skins.newSkinSubtitle") : (existing?.name ?? "")}
      icon={creating ? "add_photo_alternate" : "checkroom"}
      size="lg"
      locked={busy}
      footer={
        <>
          <Button variant="ghost" square onClick={onClose} disabled={busy}>
            {t("common.cancel")}
          </Button>
          <Button
            variant="secondary"
            square
            icon="save"
            loading={saving === "save"}
            disabled={!ready || busy}
            onClick={() => void save(false)}
          >
            {t("common.save")}
          </Button>
          <Button
            variant="primary"
            square
            icon="check"
            loading={saving === "use"}
            disabled={!ready || busy || !canApply}
            onClick={() => void save(true)}
          >
            {t("skins.saveAndUse")}
          </Button>
        </>
      }
    >
      <div className="grid grid-cols-1 sm:grid-cols-[220px_minmax(0,1fr)] gap-6">
        {/* Live preview of the draft, cape included. */}
        <div
          className="relative rounded-xl overflow-hidden h-[340px]"
          style={{
            border: "1px solid color-mix(in srgb, var(--border) 60%, transparent)",
            background: "color-mix(in srgb, var(--bg-deep) 70%, transparent)",
          }}
        >
          <div
            className="absolute inset-0 pointer-events-none"
            style={{
              background:
                "radial-gradient(ellipse 60% 50% at 50% 60%, rgba(34,197,94,0.14), rgba(34,211,238,0.05) 40%, transparent 70%)",
            }}
          />
          <SkinViewer3D
            skin={texture}
            cape={capeTexture}
            model={variant === "slim" ? "slim" : "default"}
            autoRotate={false}
            zoom={0.72}
          />
          {!texture ? (
            <p
              className="absolute inset-x-3 bottom-3 text-[11px] text-center leading-snug"
              style={{ color: "var(--text-meta)" }}
            >
              {t("skins.noFileYet")}
            </p>
          ) : null}
        </div>

        <div className="space-y-5">
          <Field label={t("skins.name")} icon="label" htmlFor="skin-name">
            <Input
              id="skin-name"
              value={name}
              maxLength={40}
              placeholder={t("skins.namePlaceholder")}
              onChange={(event) => setName(event.target.value)}
              autoFocus
            />
          </Field>

          <Field label={t("skins.playerModel")} icon="accessibility_new" hint={t("skins.variantHint")}>
            <div className="grid grid-cols-2 gap-2" role="radiogroup" aria-label={t("skins.playerModel")}>
              {(["classic", "slim"] as SkinVariant[]).map((value) => (
                <button
                  key={value}
                  type="button"
                  role="radio"
                  aria-checked={variant === value}
                  className="option-card items-center gap-2.5 py-2.5"
                  onClick={() => setVariant(value)}
                >
                  <Radio checked={variant === value} />
                  <span className="text-sm font-medium" style={{ color: "var(--text-primary)" }}>
                    {t(`skins.variants.${value}`)}
                  </span>
                </button>
              ))}
            </div>
          </Field>

          <Field label={t("skins.file")} icon="image" hint={t("skins.fileHint")}>
            <div className="flex items-center gap-3 flex-wrap">
              <Button variant="secondary" square icon="folder_open" size="sm" onClick={() => void browse()} disabled={busy}>
                {t("skins.browse")}
              </Button>
              <span className="text-xs truncate min-w-0" style={{ color: "var(--text-meta)" }}>
                {pickedPath ? fileName(pickedPath) : texture ? t("skins.fileKept") : t("skins.fileNone")}
              </span>
            </div>
          </Field>

          {capes.length > 0 ? (
            <Field label={t("skins.cape")} icon="checkroom">
              <div className="grid grid-cols-3 sm:grid-cols-4 gap-2" role="radiogroup" aria-label={t("skins.cape")}>
                <CapeChoice checked={capeId === null} label={t("skins.capeNone")} onSelect={() => setCapeId(null)} />
                {capes.map((cape) => (
                  <CapeChoice
                    key={cape.id}
                    checked={capeId === cape.id}
                    label={cape.alias ?? t("skins.cape")}
                    texture={cape.texture}
                    onSelect={() => setCapeId(cape.id)}
                  />
                ))}
              </div>
            </Field>
          ) : null}
        </div>
      </div>
    </Modal>
  );
}

function Radio({ checked }: { checked: boolean }) {
  return (
    <span
      className="w-4 h-4 rounded-full border flex items-center justify-center shrink-0"
      style={{
        borderColor: checked ? "var(--brand-primary)" : "var(--border-strong)",
        background: checked ? "var(--brand-primary)" : "transparent",
      }}
    >
      {checked ? <span className="w-1.5 h-1.5 rounded-full" style={{ background: "var(--on-brand)" }} /> : null}
    </span>
  );
}

interface CapeChoiceProps {
  checked: boolean;
  label: string;
  texture?: string;
  onSelect: () => void;
}

function CapeChoice({ checked, label, texture, onSelect }: CapeChoiceProps) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={checked}
      className="flex flex-col items-center gap-2 rounded-xl px-2 py-2.5 transition-all duration-200"
      style={{
        background: checked ? "rgba(34,197,94,0.1)" : "color-mix(in srgb, var(--bg-deep) 60%, transparent)",
        border: `1px solid ${checked ? "rgba(34,197,94,0.45)" : "color-mix(in srgb, var(--border) 60%, transparent)"}`,
        boxShadow: checked ? "var(--glow-sm)" : undefined,
      }}
      onClick={onSelect}
    >
      {texture ? (
        <CapeImage texture={texture} height={72} title={label} />
      ) : (
        <span
          className="flex items-center justify-center rounded-md"
          style={{
            height: 72,
            width: 45,
            border: "1px dashed var(--border-strong)",
            color: "var(--text-meta)",
          }}
        >
          <Icon name="block" size={18} />
        </span>
      )}
      <span className="flex items-center gap-1.5 max-w-full">
        <Radio checked={checked} />
        <span className="text-[11px] truncate" style={{ color: "var(--text-label)" }}>
          {label}
        </span>
      </span>
    </button>
  );
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

function fileStem(path: string): string {
  return fileName(path).replace(/\.png$/i, "");
}
