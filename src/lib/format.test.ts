import { describe, expect, it } from "vitest";
import { bytes, clock, compactCount, progressPair, relativeDate, speed } from "./format";

describe("format", () => {
  it("bytes", () => {
    expect(bytes(0)).toBe("0 B");
    expect(bytes(1023)).toBe("1023 B");
    expect(bytes(1024)).toBe("1.0 KB");
    expect(bytes(128 * 1024 * 1024)).toBe("128.0 MB");
    expect(bytes(null)).toBe("—");
    expect(bytes(-1)).toBe("—");
  });

  it("progress pair shares the total's unit", () => {
    expect(progressPair(41.2 * 1024 * 1024, 128 * 1024 * 1024)).toBe("41.2 / 128.0 MB");
    expect(progressPair(500, 2 * 1024 * 1024 * 1024)).toBe("0.0 / 2.0 GB");
    expect(progressPair(2048, null)).toBe("2.0 KB");
  });

  it("speed and clock", () => {
    expect(speed(0)).toBe("—");
    expect(speed(12.4 * 1024 * 1024)).toBe("12.4 MB/s");
    expect(clock(7)).toBe("0:07");
    expect(clock(3725)).toBe("1:02:05");
    expect(clock(null)).toBe("—");
  });

  it("counts and dates", () => {
    expect(compactCount(1_234_567)).toBe("1.2M");
    expect(compactCount(999)).toBe("999");
    const now = Date.UTC(2026, 9, 22, 12);
    expect(relativeDate(now - 5_000, now)).toBe("just now");
    expect(relativeDate(now - 7_200_000, now)).toBe("2 h ago");
    expect(relativeDate(Date.UTC(2026, 0, 2), now)).toBe("2026-01-02");
  });
});
