import { describe, expect, it } from "vitest";

import { describeError, t } from "../../i18n";

describe("i18n", () => {
  it("interpolates parameters and plurals", () => {
    expect(t("home.playersOnline", { count: 1 })).toBe("1 joueur en ligne");
    expect(t("home.playersOnline", { count: 3 })).toBe("3 joueurs en ligne");
    expect(t("update.banner", { version: "1.2.0" })).toBe("Mise à jour 1.2.0 disponible");
  });

  it("returns the key when missing", () => {
    expect(t("does.not.exist")).toBe("does.not.exist");
  });

  it("describes backend errors by code", () => {
    expect(describeError({ code: "network", message: "x" })).toContain("connexion");
    expect(describeError({ code: "api_error", message: "quota" })).toContain("quota");
    expect(describeError({ code: "weird", message: "raw message" })).toBe("raw message");
    expect(describeError(null)).toBe("Erreur inattendue.");
  });
});
