import { computed, onMounted, onUnmounted, ref, watch } from "vue";

export const MIN_UI_SCALE = 0.82;
export const MAX_UI_SCALE = 1.0;
export const UI_SCALE_STEP = 0.02;
export const STORAGE_KEY = "ui_scale";

const rawScale = ref(readStoredScale());
const initialized = ref(false);
let mountedConsumers = 0;

function clampScale(value: number): number {
  if (!Number.isFinite(value)) return MAX_UI_SCALE;
  const stepped = Math.round(value / UI_SCALE_STEP) * UI_SCALE_STEP;
  return Number(Math.min(MAX_UI_SCALE, Math.max(MIN_UI_SCALE, stepped)).toFixed(2));
}

function readStoredScale(): number {
  if (typeof window === "undefined") return MAX_UI_SCALE;
  const stored = Number(window.localStorage.getItem(STORAGE_KEY) ?? `${MAX_UI_SCALE}`);
  return clampScale(stored);
}

function applyScale(scale: number) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.style.setProperty("--ui-scale", String(scale));
  root.style.fontSize = `${14 * scale}px`;
  root.dataset.uiScale = scale.toFixed(2);
}

function persistScale(scale: number) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(STORAGE_KEY, scale.toFixed(2));
}

function shouldIgnoreShortcut(event: KeyboardEvent): boolean {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toLowerCase();
  return tag === "input" || tag === "textarea" || tag === "select" || target.isContentEditable;
}

watch(
  rawScale,
  (value) => {
    const next = clampScale(value);
    if (next !== value) {
      rawScale.value = next;
      return;
    }
    applyScale(next);
    persistScale(next);
  },
  { immediate: true },
);

export function useUiScale() {
  const scale = computed(() => clampScale(rawScale.value));
  const scalePercent = computed(() => `${Math.round(scale.value * 100)}%`);

  function setScale(value: number) {
    rawScale.value = clampScale(value);
  }

  function increment() {
    setScale(scale.value + UI_SCALE_STEP);
  }

  function decrement() {
    setScale(scale.value - UI_SCALE_STEP);
  }

  function reset() {
    setScale(MAX_UI_SCALE);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
    if (shouldIgnoreShortcut(event)) return;

    if (event.key === "=" || event.key === "+") {
      event.preventDefault();
      increment();
      return;
    }

    if (event.key === "-") {
      event.preventDefault();
      decrement();
      return;
    }

    if (event.key === "0") {
      event.preventDefault();
      reset();
    }
  }

  onMounted(() => {
    if (!initialized.value) {
      applyScale(scale.value);
      initialized.value = true;
    }
    if (mountedConsumers === 0) {
      window.addEventListener("keydown", handleKeydown);
    }
    mountedConsumers += 1;
  });

  onUnmounted(() => {
    mountedConsumers = Math.max(0, mountedConsumers - 1);
    if (mountedConsumers === 0) {
      window.removeEventListener("keydown", handleKeydown);
    }
  });

  return {
    scale,
    scalePercent,
    setScale,
    increment,
    decrement,
    reset,
  };
}
