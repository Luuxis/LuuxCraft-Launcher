import { describe, expect, it } from "vitest";

import { formatBytes, formatDuration, formatMemory, formatPercent, formatSpeed, relativeTime, textPreview } from "../format";

describe("format", () => {
  it("formats bytes in French units", () => {
    expect(formatBytes(0)).toBe("0 o");
    expect(formatBytes(512)).toBe("512 o");
    expect(formatBytes(1536)).toBe("1,5 Ko");
    expect(formatBytes(2 * 1024 * 1024)).toBe("2,0 Mo");
    expect(formatSpeed(1024 * 1024)).toBe("1,0 Mo/s");
  });

  it("formats durations", () => {
    expect(formatDuration(5)).toBe("5 s");
    expect(formatDuration(65)).toBe("1 min 05 s");
    expect(formatDuration(3700)).toBe("1 h 01 min");
    expect(formatDuration(-1)).toBe("—");
  });

  it("formats memory", () => {
    expect(formatMemory(512)).toBe("512 Mo");
    expect(formatMemory(1024)).toBe("1 Go");
    expect(formatMemory(1536)).toBe("1,5 Go");
  });

  it("clamps percentages", () => {
    expect(formatPercent(50, 200)).toBe(25);
    expect(formatPercent(10, 0)).toBe(0);
    expect(formatPercent(300, 200)).toBe(100);
  });

  it("computes relative times", () => {
    const now = 1_700_000_000_000;
    expect(relativeTime(now / 1000 - 10, now)).toBe("à l'instant");
    expect(relativeTime(now / 1000 - 600, now)).toBe("il y a 10 min");
    expect(relativeTime(now / 1000 - 7200, now)).toBe("il y a 2 h");
  });

  it("strips html for previews", () => {
    expect(textPreview("<h1>hello&nbsp;le&nbsp;monde&nbsp;!</h1><script>x()</script>")).toBe("hello le monde !");
    expect(textPreview("a".repeat(300), 20)).toBe(`${"a".repeat(20)}…`);
  });
});
