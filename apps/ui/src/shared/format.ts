export function formatDate(iso: string): string {
  if (!iso) return "--";
  return new Date(iso).toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

/**
 * 本地时间「昨天」的结束时刻，输出为 ISO 字符串（UTC）。
 *
 * 业务口径要求“近 30 天”严格按 T-30 00:00:00 ～ T-1 23:59:59 计算，
 * 不包含今天，因此这里返回的是本地昨天 23:59:59。
 *
 * 示例：本地今天 2026-04-19 → 返回 "2026-04-18T15:59:59.999Z"
 * （= 本地 2026-04-18 23:59:59）
 */
export function localYesterdayEndIso(): string {
  const now = new Date();
  now.setDate(now.getDate() - 1);
  now.setHours(23, 59, 59, 999);
  return now.toISOString();
}

/**
 * 本地时间「N 天前」的当天开始 (00:00:00)，输出为 ISO 字符串（UTC）。
 *
 * 与 [`localYesterdayEndIso`] 配对使用，组合起来表达
 * “最近 N 天（按本地日历，且不含今天）”。
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
