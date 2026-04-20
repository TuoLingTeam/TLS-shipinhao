// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getReviewRangeFromPreset } from "./format";

describe("getReviewRangeFromPreset", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 3, 20, 12, 0, 0));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("maps presets to local calendar windows (示例：2026-04-20 为今天)", () => {
    const anchor = new Date(2026, 3, 20, 12, 0, 0);

    expect(getReviewRangeFromPreset("today", anchor)).toMatchObject({
      startYmd: "2026-04-20",
      endYmd: "2026-04-20",
      days: 1,
    });

    expect(getReviewRangeFromPreset("yesterday", anchor)).toMatchObject({
      startYmd: "2026-04-19",
      endYmd: "2026-04-19",
      days: 1,
    });

    expect(getReviewRangeFromPreset("last_7_days", anchor)).toMatchObject({
      startYmd: "2026-04-13",
      endYmd: "2026-04-19",
      days: 7,
    });

    expect(getReviewRangeFromPreset("last_30_days", anchor)).toMatchObject({
      startYmd: "2026-03-21",
      endYmd: "2026-04-19",
      days: 30,
    });
  });

  it("returns ISO endpoints covering full local days", () => {
    const anchor = new Date(2026, 3, 20, 12, 0, 0);
    const r = getReviewRangeFromPreset("today", anchor);
    expect(r.startAt).toBe(new Date(2026, 3, 20, 0, 0, 0, 0).toISOString());
    expect(r.endAt).toBe(new Date(2026, 3, 20, 23, 59, 59, 999).toISOString());
  });
});
