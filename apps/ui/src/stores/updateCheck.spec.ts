// @vitest-environment jsdom

import { createPinia, setActivePinia } from "pinia";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useUpdateCheckStore } from "./updateCheck";

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

function sampleInfo(over: Partial<{ has_update: boolean; mandatory: boolean; version: string }> = {}) {
  return {
    app: "TLS-shipinhao",
    version: over.version ?? "9.9.9",
    build: 1,
    mandatory: over.mandatory ?? false,
    platform: "mac",
    download_url: "https://example.com/dl",
    tutorial_url: "",
    notes: [] as string[],
    has_update: over.has_update ?? true,
    raw_payload: {},
  };
}

describe("useUpdateCheckStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    localStorage.clear();
    invokeMock.mockReset();
    listenMock.mockReset();
    listenMock.mockResolvedValue(vi.fn());
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("refresh 成功后写入 latestInfo 且横幅默认可见", async () => {
    invokeMock.mockResolvedValueOnce(sampleInfo({ has_update: true, version: "5.3.0" }));
    const store = useUpdateCheckStore();
    await store.refresh();
    expect(store.latestInfo?.version).toBe("5.3.0");
    expect(store.hasUpdateAvailable).toBe(true);
    expect(store.bannerVisible).toBe(true);
  });

  it("稍后提醒会让 bannerVisible 为 false 但 hasUpdateAvailable 仍为 true", async () => {
    invokeMock.mockResolvedValueOnce(sampleInfo({ has_update: true }));
    const store = useUpdateCheckStore();
    await store.refresh();
    store.dismissBanner();
    expect(store.hasUpdateAvailable).toBe(true);
    expect(store.bannerVisible).toBe(false);
    expect(store.isSnoozed).toBe(true);
  });

  it("clearSnooze 后横幅重新满足可见条件", async () => {
    invokeMock.mockResolvedValueOnce(sampleInfo({ has_update: true }));
    const store = useUpdateCheckStore();
    await store.refresh();
    store.dismissBanner();
    expect(store.bannerVisible).toBe(false);
    store.clearSnooze();
    expect(store.bannerVisible).toBe(true);
  });

  it("has_update 为 false 时清空可更新态（修复旧版仅在有更新时写入导致的陈旧状态）", async () => {
    invokeMock.mockResolvedValueOnce(sampleInfo({ has_update: true, version: "5.3.0" }));
    const store = useUpdateCheckStore();
    await store.refresh();
    invokeMock.mockResolvedValueOnce(sampleInfo({ has_update: false, version: "5.3.0" }));
    await store.refresh();
    expect(store.hasUpdateAvailable).toBe(false);
    expect(store.bannerVisible).toBe(false);
  });

  it("首次失败后延迟重试成功会清除 lastError", async () => {
    vi.useFakeTimers();
    invokeMock.mockRejectedValueOnce(new Error("net down"));
    invokeMock.mockResolvedValueOnce(sampleInfo({ has_update: true }));
    const store = useUpdateCheckStore();
    const p = store.refresh();
    await vi.advanceTimersByTimeAsync(3000);
    await p;
    expect(store.latestInfo?.has_update).toBe(true);
    expect(store.lastError).toBeNull();
  });

  it("openDownloadUrl 使用后端返回的 download_url 调用 open_external_url", async () => {
    invokeMock.mockResolvedValueOnce(sampleInfo({ has_update: true }));
    invokeMock.mockResolvedValueOnce(undefined);
    const store = useUpdateCheckStore();
    await store.refresh();
    await store.openDownloadUrl();
    expect(invokeMock).toHaveBeenLastCalledWith("open_external_url", { url: "https://example.com/dl" });
    expect(store.downloadActionError).toBeNull();
  });

  it("openTutorialUrl 使用 tutorial_url 调用 open_external_url", async () => {
    invokeMock.mockResolvedValueOnce({
      ...sampleInfo({ has_update: false }),
      tutorial_url: "https://example.com/howto",
    });
    invokeMock.mockResolvedValueOnce(undefined);
    const store = useUpdateCheckStore();
    await store.refresh();
    await store.openTutorialUrl();
    expect(invokeMock).toHaveBeenLastCalledWith("open_external_url", { url: "https://example.com/howto" });
    expect(store.tutorialActionError).toBeNull();
  });
});
