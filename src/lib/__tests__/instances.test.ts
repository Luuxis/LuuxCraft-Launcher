import { describe, expect, it } from "vitest";

import { allowedInstances, instanceAllows, loaderLabel, moduleEnabled, pickInstance } from "../instances";
import type { Instance } from "../types";

function instance(overrides: Partial<Instance>): Instance {
  return {
    id: "id",
    name: "Instance",
    description: null,
    image: null,
    filesUrl: null,
    minecraftVersion: "1.21.1",
    loader: { kind: "neoforge", version: "latest", mcpFile: null },
    verify: false,
    ignored: [],
    whitelist: [],
    whitelistActive: false,
    server: null,
    javaVersion: null,
    jvmArgs: [],
    memory: null,
    order: null,
    extra: {},
    ...overrides,
  };
}

describe("instances", () => {
  it("applies the panel whitelist", () => {
    const open = instance({ id: "a" });
    const restricted = instance({ id: "b", whitelistActive: true, whitelist: ["Luuxis"] });
    expect(instanceAllows(open, null)).toBe(true);
    expect(instanceAllows(restricted, null)).toBe(false);
    expect(instanceAllows(restricted, "Luuxis")).toBe(true);
    expect(instanceAllows(restricted, "luuxis")).toBe(false);
    expect(allowedInstances([open, restricted], "Steve").map((i) => i.id)).toEqual(["a"]);
  });

  it("picks the remembered instance when still allowed", () => {
    const list = [instance({ id: "a" }), instance({ id: "b" })];
    expect(pickInstance(list, "b")?.id).toBe("b");
    expect(pickInstance(list, "zzz")?.id).toBe("a");
    expect(pickInstance([], "a")).toBeNull();
  });

  it("labels loaders", () => {
    expect(loaderLabel("neoforge")).toBe("NeoForge");
    expect(loaderLabel("none")).toBe("Vanilla");
    expect(loaderLabel("custom")).toBe("Custom");
  });

  it("merges module toggles", () => {
    const defaults = { news: true, skins: true };
    expect(moduleEnabled("news", defaults, undefined)).toBe(true);
    expect(moduleEnabled("news", defaults, { news: false })).toBe(false);
    expect(moduleEnabled("news", defaults, { news: { enabled: false } })).toBe(false);
    expect(moduleEnabled("news", defaults, { news: "off" })).toBe(false);
    expect(moduleEnabled("unknown", defaults, {})).toBe(true);
  });
});
