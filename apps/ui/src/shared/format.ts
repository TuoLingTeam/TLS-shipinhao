export function formatDate(iso: string): string {
  if (!iso) return "--";
  return new Date(iso).toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

/**
 * 本地时间「今天」的结束时刻，输出为 ISO 字符串（UTC）。
 *
 * 背景：以前用 `new Date().toISOString().split("T")[0]T23:59:59Z` 取 UTC 当天
 * 23:59:59，本地（+8）显示出来会变成次日 07:59:59，导致订单管理页「覆盖
 * 区间」永远比日历多出一天。改用本地时间的当天结束 → toISOString（转成 UTC）
 * 后，后端拿到的 Unix 秒对应真实的"本地今天"边界，界面上再格式化回本地
 * 时区就与日历一致。
 *
 * 示例：本地 2026-04-18 14:00 +08 → 返回 "2026-04-18T15:59:59.999Z"
 * （= 本地 2026-04-18 23:59:59）
 */
export function localTodayEndIso(): string {
  const now = new Date();
  now.setHours(23, 59, 59, 999);
  return now.toISOString();
}

/**
 * 本地时间「N 天前」的当天开始 (00:00:00)，输出为 ISO 字符串（UTC）。
 *
 * 与 [`localTodayEndIso`] 配对使用，组合起来表达"最近 N 天（按本地日历）全量订单"。
 *
 * 示例：本地 2026-04-18、N=30 → 3 月 19 日起算，返回 "2026-03-18T16:00:00.000Z"
 * （= 本地 2026-03-19 00:00:00）
 */
export function localDaysAgoStartIso(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() - days);
  d.setHours(0, 0, 0, 0);
  return d.toISOString();
}

export function formatCent(cent: number): string {
  return `¥${(cent / 100).toFixed(2)}`;
}


export function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return "-";
  return new Date(iso).toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}
