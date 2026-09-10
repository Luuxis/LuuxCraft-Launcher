import { describe, expect, it } from "vitest";

import { presentLink } from "../links";

describe("presentLink", () => {
  it("recognizes brands from the icon key", () => {
    const discord = presentLink({ label: "Notre Discord", url: "https://example.com", icon: "discord", order: null });
    expect(discord.brandPath).toBeTruthy();
    expect(discord.brandColor).toBe("#5865F2");
  });

  it("recognizes brands from the host", () => {
    const yt = presentLink({ label: "Chaîne", url: "https://www.youtube.com/@luuxcraft", icon: null, order: null });
    expect(yt.brandPath).toBeTruthy();
    const x = presentLink({ label: "Twitter", url: "https://x.com/luuxcraft", icon: null, order: null });
    expect(x.brandPath).toBeTruthy();
  });

  it("falls back to material symbols", () => {
    expect(presentLink({ label: "Boutique", url: "https://shop.example", icon: "shop", order: null }).symbol).toBe("shopping_bag");
    expect(presentLink({ label: "Site web", url: "https://luuxcraft.fr", icon: "website", order: null }).symbol).toBe("language");
    expect(presentLink({ label: "Autre", url: "https://autre.example", icon: null, order: null }).symbol).toBe("link");
    expect(presentLink({ label: "Custom", url: "https://c.example", icon: "rocket_launch", order: null }).symbol).toBe("rocket_launch");
  });
});
