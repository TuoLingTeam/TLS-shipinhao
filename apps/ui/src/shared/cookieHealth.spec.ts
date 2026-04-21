// @vitest-environment jsdom

import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useCookieHealthStore } from "./cookieHealth";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("useCookieHealthStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
  });

  it("初始 snapshot 默认值且 status 为 unknown", () => {
    const store = useCookieHealthStore();
    expect(store.snapshot.healthy).toBe(false);
    expect(store.snapshot.configured).toBe(false);
    expect(store.snapshot.has_biz_magic).toBe(false);
    expect(store.snapshot.last_checked_at).toBeNull();
    expect(store.status).toBe("unknown");
  });

  it("refreshSilently 成功会写入后端返回的 snapshot 并调用 get_cookie_health 一次", async () => {
    invokeMock.mockResolvedValueOnce({
      healthy: true,
      configured: true,
      has_biz_magic: true,
      last_checked_at: "2026-04-20T10:00:00Z",
      hint: "ok",
    });
    const store = useCookieHealthStore();

    await store.refreshSilently();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("get_cookie_health");
    expect(store.snapshot.healthy).toBe(true);
    expect(store.status).toBe("healthy");
  });

  it("refreshSilently 失败只记 error，不改 snapshot", async () => {
    invokeMock.mockRejectedValueOnce("network down");
    const store = useCookieHealthStore();
    const before = { ...store.snapshot };

    await store.refreshSilently();

    expect(store.error).toBe("network down");
    expect(store.snapshot).toEqual(before);
  });

  it("probe 成功写入 snapshot，并在进行中 loading=true", async () => {
    let resolveInvoke: (value: unknown) => void = () => {};
    invokeMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveInvoke = resolve;
        }),
    );
    const store = useCookieHealthStore();
    const task = store.probe();
    // 等一次 microtask，让 loading.value 被置 true
    await Promise.resolve();
    expect(store.loading).toBe(true);

    resolveInvoke({
      healthy: false,
      configured: true,
      has_biz_magic: false,
      last_checked_at: "2026-04-20T10:05:00Z",
      hint: "biz_magic 缺失",
    });
    await task;

    expect(store.loading).toBe(false);
    expect(store.status).toBe("unhealthy");
  });

  it("status computed 覆盖 unconfigured 分支", async () => {
    invokeMock.mockResolvedValueOnce({
      healthy: false,
      configured: false,
      has_biz_magic: false,
      last_checked_at: "2026-04-20T10:00:00Z",
      hint: "尚未配置 Cookie",
    });
    const store = useCookieHealthStore();
    await store.refreshSilently();
    expect(store.status).toBe("unconfigured");
  });

  it("probe 进行中再次调用会被直接忽略（loading 守门）", async () => {
    let resolveFirst: (value: unknown) => void = () => {};
    invokeMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
    );
    const store = useCookieHealthStore();

    const first = store.probe();
    await Promise.resolve();
    // 第二次调用应该直接返回，不再触发 invoke
    const second = store.probe();
    await second;
    expect(invokeMock).toHaveBeenCalledTimes(1);

    resolveFirst({
      healthy: true,
      configured: true,
      has_biz_magic: true,
      last_checked_at: "2026-04-20T10:00:00Z",
      hint: "ok",
    });
    await first;
  });
});
