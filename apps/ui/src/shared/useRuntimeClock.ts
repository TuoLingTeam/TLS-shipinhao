import { computed, onBeforeUnmount, onMounted, ref, type ComputedRef } from "vue";

/**
 * 应用启动时间。
 *
 * 放在模块级以保证全局唯一：用户即便在不同路由/组件间切换，
 * 「会话时长」也应以首次加载为起点，而不是每次挂载重置。
 */
const appStartedAt = new Date();

/** 全局共享的「当前时间」ref，所有消费者读同一个值，避免多组件定时器漂移。 */
const sharedNow = ref(new Date());

/** 30 秒粒度足够驱动「hh:mm」与「已运行 Nh Mm」两类展示，避免频繁重渲染。 */
const TICK_INTERVAL_MS = 30_000;

/** 引用计数：0 → 1 启动定时器；1 → 0 停止定时器，避免后台空跑。 */
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
  /** 底层启动时间（Date），便于外部二次派生。 */
  appStartedAt: Date;
}

/**
 * 获取运行时时钟（启动时间 / 当前时间）。
 *
 * 首次被任意组件调用时启动 30s 定时器，全部组件卸载后自动停止。
 * 多处消费读取同一 ref，保证 UI 在同一帧内显示一致的时间。
 */
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

  return { clockText, uptimeText, appStartedAt };
}
