import { computed, onBeforeUnmount, onMounted, ref, type ComputedRef } from "vue";

/** 模块级启动时间，保证跨路由的会话时长不重置。 */
const appStartedAt = new Date();

/** 全局共享当前时间，避免多组件定时器漂移。 */
const sharedNow = ref(new Date());

/** 30 秒粒度足够驱动时钟与运行时长展示。 */
const TICK_INTERVAL_MS = 30_000;

/** 引用计数控制共享定时器生命周期。 */
let refCount = 0;
let timer: ReturnType<typeof setInterval> | null = null;

function acquireTimer(): void {
  refCount += 1;
  if (timer !== null) return;
  timer = setInterval(() => {
    sharedNow.value = new Date();
  }, TICK_INTERVAL_MS);
}

function releaseTimer(): void {
  refCount = Math.max(0, refCount - 1);
  if (refCount > 0 || timer === null) return;
  clearInterval(timer);
  timer = null;
}

export interface RuntimeClock {
  /** 当前时间 hh:mm（零填充，24 小时制），适合状态卡展示。 */
  clockText: ComputedRef<string>;
  /** 会话累计时长：刚启动显示「刚刚启动」，不足 1 小时显示分钟，否则「Nh Mm」。 */
  uptimeText: ComputedRef<string>;
}

/** 获取运行时时钟，按需启动共享定时器。 */
export function useRuntimeClock(): RuntimeClock {
  onMounted(() => {
    acquireTimer();
  });
  onBeforeUnmount(() => {
    releaseTimer();
  });

  const clockText = computed(() => {
    const hh = String(sharedNow.value.getHours()).padStart(2, "0");
    const mm = String(sharedNow.value.getMinutes()).padStart(2, "0");
    return `${hh}:${mm}`;
  });

  const uptimeText = computed(() => {
    const diffMs = sharedNow.value.getTime() - appStartedAt.getTime();
    const totalMin = Math.max(0, Math.floor(diffMs / 60_000));
    const hours = Math.floor(totalMin / 60);
    const minutes = totalMin % 60;
    if (hours > 0) return `已运行 ${hours}h ${minutes}m`;
    if (minutes > 0) return `已运行 ${minutes} 分钟`;
    return "刚刚启动";
  });

  return { clockText, uptimeText };
}
