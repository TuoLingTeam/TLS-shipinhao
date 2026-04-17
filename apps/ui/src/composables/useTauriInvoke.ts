import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export function useTauriInvoke<T>(command: string) {
  const data = ref<T | null>(null);
  const error = ref<string | null>(null);
  const loading = ref(false);

  async function execute(args?: Record<string, unknown>): Promise<T | null> {
    loading.value = true;
    error.value = null;
    try {
      const result = await invoke<T>(command, args);
      data.value = result;
      return result;
    } catch (e) {
      error.value = typeof e === "string" ? e : String(e);
      return null;
    } finally {
      loading.value = false;
    }
  }

  return { data, error, loading, execute };
}
