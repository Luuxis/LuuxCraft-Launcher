/**
 * Premier lancement d'un launcher générique qui n'a pas trouvé sa
 * configuration dans l'installeur.
 *
 * Le cas normal est que cet écran ne s'affiche jamais : le panel ajoute la
 * configuration à la fin de l'installeur au téléchargement et le launcher la
 * lit tout seul (voir `src-tauri/src/provisioning.rs`). Il reste les formats
 * qui ne tolèrent pas d'octets ajoutés à la fin — le `.dmg` de macOS,
 * principalement — où le joueur saisit son code une seule fois.
 *
 * Seul le code est demandé : l'adresse du panel est compilée, elle est la même
 * pour tous ses clients.
 */
import { useState } from "react";

import { describeError, t } from "../../i18n";
import { useActions, useAppState } from "../../store/AppStore";
import { Button } from "../../components/ui/Button";
import { Icon } from "../../components/ui/Icon";
import { Notice } from "../../components/ui/primitives";
import { Field, Input } from "../../components/ui/forms";

export function PairingView() {
  const state = useAppState();
  const { pairLauncher } = useActions();
  const [code, setCode] = useState("");

  const panel = state.bootstrap?.provisioning.apiUrl.replace(/^https:\/\//, "").replace(/\/api$/, "") ?? "";

  return (
    <div className="relative flex-1 flex items-center justify-center p-6 sm:p-8 animate-fade-in">
      <div
        className="blob w-[600px] h-[600px] -top-40 left-1/2 -translate-x-1/2"
        style={{ background: "rgba(34,197,94,0.08)", filter: "blur(140px)" }}
      />
      <form
        className="premium-card static p-8 max-w-md w-full space-y-5"
        onSubmit={(event) => {
          event.preventDefault();
          void pairLauncher(code);
        }}
      >
        <span className="icon-chip w-14 h-14 rounded-2xl" style={{ borderRadius: 16 }}>
          <Icon name="key" size={28} />
        </span>

        <div className="space-y-2">
          <h1 className="text-2xl font-bold" style={{ color: "var(--text-primary)" }}>
            {t("pairing.title")}
          </h1>
          <p className="text-sm leading-relaxed" style={{ color: "var(--text-body)" }}>
            {t("pairing.intro")}
          </p>
        </div>

        <Field label={t("pairing.codeLabel")} icon="vpn_key" htmlFor="pairing-code" hint={t("pairing.codeHint")}>
          <Input
            id="pairing-code"
            mono
            autoFocus
            autoComplete="off"
            placeholder={t("pairing.codePlaceholder")}
            value={code}
            disabled={state.pairingBusy}
            onChange={(event) => setCode(event.currentTarget.value)}
          />
        </Field>

        {state.pairingError ? (
          <Notice tone="error" details={state.pairingError.details}>
            {describeError(state.pairingError)}
          </Notice>
        ) : null}

        <Button
          type="submit"
          variant="primary"
          size="lg"
          icon="link"
          className="w-full justify-center"
          loading={state.pairingBusy}
          disabled={!code.trim()}
        >
          {t("pairing.submit")}
        </Button>

        {panel ? (
          <p className="text-xs text-center" style={{ color: "var(--text-meta)" }}>
            {t("pairing.panel", { panel })}
          </p>
        ) : null}
      </form>
    </div>
  );
}
