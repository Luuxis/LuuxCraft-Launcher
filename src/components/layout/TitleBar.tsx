import { usePlatform } from "../../lib/platform";
import { WindowControls } from "./WindowControls";

/**
 * Barre de titre intégrée (les décorations système sont désactivées).
 *
 * Nue, comme celle d'une application native : ni logo ni nom — la fenêtre se
 * reconnaît à son contenu. Elle ne porte que la zone de déplacement et les
 * boutons de fenêtre, disposés selon le système : feux à gauche sur macOS,
 * symboles à droite ailleurs.
 */
export function TitleBar() {
  const platform = usePlatform();
  const traffic = platform === "macos";

  return (
    <header data-tauri-drag-region className="titlebar relative z-30 flex items-center shrink-0">
      {traffic ? <WindowControls kind="traffic" /> : null}
      {/* Tauri ne déclenche le déplacement que sur l'élément qui porte
          l'attribut : l'espace vide en a besoin lui aussi. */}
      <div data-tauri-drag-region className="flex-1 h-full" />
      {traffic ? null : <WindowControls kind="symbols" />}
    </header>
  );
}
