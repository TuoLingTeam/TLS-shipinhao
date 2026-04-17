import { computed, onMounted, onUnmounted, ref, watch } from "vue";

export const WIDE_LAYOUT_MIN_WIDTH = 1320;
export const WIDE_LAYOUT_MIN_HEIGHT = 780;
export const COMPACT_LAYOUT_MIN_WIDTH = 860;
export const HIGH_DPI_COMPACT_THRESHOLD = 120;
export const VERY_HIGH_DPI_COMPACT_THRESHOLD = 140;

export type LayoutMode = "wide" | "normal" | "compact" | "high_dpi_compact";

const width = ref(typeof window === "undefined" ? 1440 : window.innerWidth);
const height = ref(typeof window === "undefined" ? 900 : window.innerHeight);
const dpi = ref(typeof window === "undefined" ? 96 : window.devicePixelRatio * 96);
let mountedConsumers = 0;

function syncViewportState() {
  if (typeof window === "undefined") return;
  width.value = window.innerWidth;
  height.value = window.innerHeight;
  dpi.value = window.devicePixelRatio * 96;
}

const mode = computed<LayoutMode>(() => {
  if (dpi.value >= VERY_HIGH_DPI_COMPACT_THRESHOLD) return "high_dpi_compact";
  if (width.value >= WIDE_LAYOUT_MIN_WIDTH && height.value >= WIDE_LAYOUT_MIN_HEIGHT) return "wide";
  if (width.value < COMPACT_LAYOUT_MIN_WIDTH || dpi.value >= HIGH_DPI_COMPACT_THRESHOLD) return "compact";
  return "normal";
});

watch(
  mode,
  (value) => {
    if (typeof document === "undefined") return;
    document.documentElement.dataset.layout = value;
  },
  { immediate: true },
);

export function useLayout() {
  onMounted(() => {
    syncViewportState();
    if (mountedConsumers === 0) {
      window.addEventListener("resize", syncViewportState);
    }
    mountedConsumers += 1;
  });

  onUnmounted(() => {
    mountedConsumers = Math.max(0, mountedConsumers - 1);
    if (mountedConsumers === 0) {
      window.removeEventListener("resize", syncViewportState);
    }
  });

  return {
    mode,
    width: computed(() => width.value),
    height: computed(() => height.value),
    dpi: computed(() => dpi.value),
  };
}
