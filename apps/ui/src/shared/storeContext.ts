import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { useDeliveryStore } from "../delivery/store";
import { useOrderStore } from "../order/store";
import type { OrderCacheCounts, OrderCacheStatus } from "../order/types";
import { useReviewStore } from "../review/store";
import { toErrorMessage } from "./toErrorMessage";
import { useCookieHealthStore } from "./cookieHealth";

export interface StoreMeta {
  store_id: string;
  store_name: string;
}

interface CookieStatusResponse {
  configured: boolean;
  has_biz_magic: boolean;
  cookie_path: string;
  active_store: StoreMeta | null;
  stores: StoreMeta[];
}

interface SelectStoreResponse {
  success: boolean;
  store: StoreMeta;
  configured: boolean;
  has_biz_magic: boolean;
  cookie_path: string;
}

export const useStoreContextStore = defineStore("store-context", () => {
  const stores = ref<StoreMeta[]>([]);
  const activeStore = ref<StoreMeta | null>(null);
  const cookieConfigured = ref(false);
  const hasBizMagic = ref(false);
  const cookiePath = ref("");
  const loading = ref(false);
  const error = ref<string | null>(null);

  const orderStore = useOrderStore();
  const reviewStore = useReviewStore();
  const deliveryStore = useDeliveryStore();
  const cookieHealth = useCookieHealthStore();

  const activeStoreId = computed(() => activeStore.value?.store_id ?? "");
  const activeStoreName = computed(() => activeStore.value?.store_name ?? "未选择店铺");
  const hasStores = computed(() => stores.value.length > 0);
  const busy = computed(
    () =>
      loading.value ||
      orderStore.loading ||
      reviewStore.loading ||
      deliveryStore.loading ||
      deliveryStore.batchProgress?.running === true,
  );

  function applyCookieStatus(status: CookieStatusResponse) {
    stores.value = status.stores ?? [];
    activeStore.value = status.active_store ?? null;
    cookieConfigured.value = status.configured;
    hasBizMagic.value = status.has_biz_magic;
    cookiePath.value = status.cookie_path;
  }

  function applySelection(result: SelectStoreResponse) {
    activeStore.value = result.store;
    cookieConfigured.value = result.configured;
    hasBizMagic.value = result.has_biz_magic;
    cookiePath.value = result.cookie_path;
    if (!stores.value.some((store) => store.store_id === result.store.store_id)) {
      stores.value = [...stores.value, result.store];
      return;
    }
    stores.value = stores.value.map((store) =>
      store.store_id === result.store.store_id ? result.store : store,
    );
  }

  function resetDomainState() {
    orderStore.reset();
    reviewStore.reset();
    deliveryStore.resetForStoreSwitch();
  }

  async function refresh() {
    loading.value = true;
    error.value = null;
    try {
      const status = await invoke<CookieStatusResponse>("get_cookie_status");
      applyCookieStatus(status);
      return status;
    } catch (err) {
      error.value = toErrorMessage(err);
      throw err;
    } finally {
      loading.value = false;
    }
  }

  async function refreshOrderCacheStatus() {
    try {
      const status = await invoke<OrderCacheStatus>("get_order_cache_status");
      orderStore.setCacheStatus(status);
      const counts = await invoke<OrderCacheCounts>("get_order_cache_counts");
      orderStore.setCacheCounts(counts);
    } catch (err) {
      orderStore.error = toErrorMessage(err);
    }
  }

  async function refreshAfterCookieUpdate(previousStoreId: string | null = null) {
    await refresh();
    if (previousStoreId !== activeStoreId.value) {
      resetDomainState();
    }
    await Promise.allSettled([refreshOrderCacheStatus(), cookieHealth.refreshSilently()]);
  }

  async function selectStore(storeId: string) {
    const nextStoreId = storeId.trim();
    if (!nextStoreId || nextStoreId === activeStoreId.value || busy.value) {
      return null;
    }

    const previousStoreId = activeStoreId.value || null;
    loading.value = true;
    error.value = null;
    try {
      const result = await invoke<SelectStoreResponse>("select_store", {
        store_id: nextStoreId,
      });
      applySelection(result);
      if (previousStoreId !== result.store.store_id) {
        resetDomainState();
      }
      await Promise.allSettled([refreshOrderCacheStatus(), cookieHealth.refreshSilently()]);
      return result;
    } catch (err) {
      error.value = toErrorMessage(err);
      throw err;
    } finally {
      loading.value = false;
    }
  }

  return {
    stores,
    activeStore,
    activeStoreId,
    activeStoreName,
    hasStores,
    cookieConfigured,
    hasBizMagic,
    cookiePath,
    loading,
    error,
    busy,
    refresh,
    refreshOrderCacheStatus,
    refreshAfterCookieUpdate,
    selectStore,
    resetDomainState,
  };
});
