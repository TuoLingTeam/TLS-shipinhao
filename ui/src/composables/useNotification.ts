import { ref } from "vue";

export interface Toast {
  id: number;
  message: string;
  type: "success" | "error" | "info";
}

let nextId = 0;
const toasts = ref<Toast[]>([]);

export function useNotification() {
  function show(message: string, type: Toast["type"] = "info") {
    const id = nextId++;
    toasts.value.push({ id, message, type });
    setTimeout(() => dismiss(id), 3000);
  }

  function dismiss(id: number) {
    toasts.value = toasts.value.filter((t) => t.id !== id);
  }

  return { toasts, show, dismiss };
}
