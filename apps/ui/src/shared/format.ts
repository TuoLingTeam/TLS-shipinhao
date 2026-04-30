export function formatDate(iso: string): string {
  if (!iso) return "--";
  return new Date(iso).toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

/** 本地昨天 23:59:59.999，对应“近 N 天不含今天”的结束点。 */
export function localYesterdayEndIso(): string {
  const now = new Date();
  now.setDate(now.getDate() - 1);
  now.setHours(23, 59, 59, 999);
  return now.toISOString();
}

/** 本地 N 天前 00:00:00.000，与 localYesterdayEndIso 组成不含今天的窗口。 */
export function localDaysAgoStartIso(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() - days);
  d.setHours(0, 0, 0, 0);
  return d.toISOString();
}

/** 本地日历日 YYYY-MM-DD（用于 `<input type="date">`） */
export function formatLocalYmd(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

/** 本地「昨天」的日历日字符串，与差评/品退口径一致（不含今日） */
export function localYesterdayYmd(): string {
  const d = new Date();
  d.setDate(d.getDate() - 1);
  return formatLocalYmd(d);
}

/** 本地「今天往前第 n 天」的日历日。 */
export function localDaysAgoYmd(days: number): string {
  const d = new Date();
  d.setDate(d.getDate() - days);
  return formatLocalYmd(d);
}

/** `YYYY-MM-DD` 本地日界线 00:00:00.000 → ISO */
export function localCalendarDateToStartIso(ymd: string): string {
  const [y, m, d] = ymd.split("-").map(Number);
  return new Date(y, m - 1, d, 0, 0, 0, 0).toISOString();
}

/** `YYYY-MM-DD` 本地日界线 23:59:59.999 → ISO */
export function localCalendarDateToEndIso(ymd: string): string {
  const [y, m, d] = ymd.split("-").map(Number);
  return new Date(y, m - 1, d, 23, 59, 59, 999).toISOString();
}

/** 本地日历上两日期（含首尾）之间的天数 */
export function inclusiveLocalDaysBetween(startYmd: string, endYmd: string): number {
  const [sy, sm, sd] = startYmd.split("-").map(Number);
  const [ey, em, ed] = endYmd.split("-").map(Number);
  const s = new Date(sy, sm - 1, sd);
  const e = new Date(ey, em - 1, ed);
  return Math.floor((e.getTime() - s.getTime()) / 86400000) + 1;
}

/** 评价/品退查询：快捷日期范围（本地日历） */
export type ReviewRangePresetKey = "today" | "yesterday" | "last_7_days" | "last_30_days";

export interface ReviewRangeWindow {
  startYmd: string;
  endYmd: string;
  startAt: string;
  endAt: string;
  days: number;
}

/**
 * 将预设转为 API 所需时间窗。
 *
 * - 今天 / 昨天：单日 00:00:00.000 ～ 23:59:59.999（本地）
 * - 近 7 天：昨天往前共 7 个自然日（含昨天）
 * - 近 30 天：昨天往前共 30 个自然日（含昨天）
 */
export function getReviewRangeFromPreset(preset: ReviewRangePresetKey, now: Date = new Date()): ReviewRangeWindow {
  const todayYmd = formatLocalYmd(now);

  let startYmd: string;
  let endYmd: string;

  switch (preset) {
    case "today":
      startYmd = todayYmd;
      endYmd = todayYmd;
      break;
    case "yesterday": {
      const d = new Date(now);
      d.setDate(d.getDate() - 1);
      startYmd = formatLocalYmd(d);
      endYmd = startYmd;
      break;
    }
    case "last_7_days": {
      const end = new Date(now);
      end.setDate(end.getDate() - 1);
      const start = new Date(end);
      start.setDate(start.getDate() - 6);
      startYmd = formatLocalYmd(start);
      endYmd = formatLocalYmd(end);
      break;
    }
    case "last_30_days": {
      const end = new Date(now);
      end.setDate(end.getDate() - 1);
      const start = new Date(end);
      start.setDate(start.getDate() - 29);
      startYmd = formatLocalYmd(start);
      endYmd = formatLocalYmd(end);
      break;
    }
    default: {
      const _exhaustive: never = preset;
      throw new Error(`未知日期预设：${String(_exhaustive)}`);
    }
  }

  const startAt = localCalendarDateToStartIso(startYmd);
  const endAt = localCalendarDateToEndIso(endYmd);
  const days = inclusiveLocalDaysBetween(startYmd, endYmd);
  return { startYmd, endYmd, startAt, endAt, days };
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
