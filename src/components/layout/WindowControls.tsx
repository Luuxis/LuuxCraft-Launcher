import type { CSSProperties } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { t } from "../../i18n";
import { Icon } from "../ui/Icon";

/**
 * Boutons de fenêtre, dans les deux langages que les joueurs connaissent.
 *
 * `traffic` : les trois feux de macOS, dont les glyphes n'apparaissent qu'au
 * survol du groupe — comme dans le système. `symbols` : réduire, agrandir et
 * fermer en cellules pleine hauteur, fermer virant au rouge au survol, comme
 * sous Windows. Un seul composant pour la barre intégrée et pour le widget des
 * thèmes, sinon les deux finiraient par diverger.
 */
export type ControlsKind = "traffic" | "symbols";

interface WindowControlsProps {
  kind: ControlsKind;
  showMinimize?: boolean;
  showMaximize?: boolean;
  iconSize?: number;
  /** Habillage venu d'un thème, posé sur chaque bouton puis sur celui de fermeture. */
  buttonStyle?: CSSProperties | null;
  closeStyle?: CSSProperties | null;
}

const windowApi = () => getCurrentWindow();

export function WindowControls({ kind, showMinimize = true, showMaximize = true, iconSize = 16, buttonStyle, closeStyle }: WindowControlsProps) {
  const minimize = () => void windowApi().minimize();
  const maximize = () => void windowApi().toggleMaximize();
  const close = () => void windowApi().close();

  if (kind === "traffic") {
    return (
      <div className="traffic-lights" role="group">
        <button type="button" className="traffic-light traffic-light-close" onClick={close} aria-label={t("common.quit")} title={t("common.quit")} style={{ ...buttonStyle, ...closeStyle }}>
          <svg viewBox="0 0 12 12" aria-hidden="true">
            <path d="M3.5 3.5l5 5M8.5 3.5l-5 5" />
          </svg>
        </button>
        {showMinimize ? (
          <button type="button" className="traffic-light traffic-light-minimize" onClick={minimize} aria-label={t("common.minimize")} title={t("common.minimize")} style={buttonStyle ?? undefined}>
            <svg viewBox="0 0 12 12" aria-hidden="true">
              <path d="M2.5 6h7" />
            </svg>
          </button>
        ) : (
          <span className="traffic-light traffic-light-off" aria-hidden="true" />
        )}
        {showMaximize ? (
          <button type="button" className="traffic-light traffic-light-zoom" onClick={maximize} aria-label={t("common.maximize")} title={t("common.maximize")} style={buttonStyle ?? undefined}>
            <svg viewBox="0 0 12 12" aria-hidden="true">
              <path d="M3 8.5V3h5.5M9 3.5V9H3.5" />
            </svg>
          </button>
        ) : (
          <span className="traffic-light traffic-light-off" aria-hidden="true" />
        )}
      </div>
    );
  }

  return (
    <div className="win-controls" role="group">
      {showMinimize ? (
        <button type="button" className="win-control" onClick={minimize} aria-label={t("common.minimize")} title={t("common.minimize")} style={buttonStyle ?? undefined}>
          <Icon name="remove" size={iconSize} />
        </button>
      ) : null}
      {showMaximize ? (
        <button type="button" className="win-control" onClick={maximize} aria-label={t("common.maximize")} title={t("common.maximize")} style={buttonStyle ?? undefined}>
          <Icon name="crop_square" size={iconSize} />
        </button>
      ) : null}
      <button type="button" className="win-control win-control-close" onClick={close} aria-label={t("common.quit")} title={t("common.quit")} style={{ ...buttonStyle, ...closeStyle }}>
        <Icon name="close" size={iconSize} />
      </button>
    </div>
  );
}
