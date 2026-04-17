import { defineStore } from "pinia";
import { ref } from "vue";
import type { OrderMatchResult, ReviewQuery } from "../review/review.types";

export type ReviewFetchMode = "bad_review" | "quality_refund";

export const useReviewStore = defineStore("review", () => {
  const results = ref<OrderMatchResult[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const lastQuery = ref<ReviewQuery | null>(null);
  const lastMode = ref<ReviewFetchMode>("bad_review");
  const cacheWarnings = ref<string[]>([]);
  const cacheCoverageStart = ref<string | null>(null);
  const cacheCoverageEnd = ref<string | null>(null);
  const cacheSyncPerformed = ref(false);
  const cacheSyncWrittenCount = ref(0);

  function setResults(data: OrderMatchResult[]) {
    results.value = data;
  }

  function setLoading(val: boolean) {
    loading.value = val;
  }

  function setError(msg: string | null) {
    error.value = msg;
  }

  function setLastQuery(query: ReviewQuery) {
    lastQuery.value = query;
  }

  function setLastMode(mode: ReviewFetchMode) {
    lastMode.value = mode;
  }

  return {
    results,
    loading,
    error,
    lastQuery,
    lastMode,
    cacheWarnings,
    cacheCoverageStart,
    cacheCoverageEnd,
    cacheSyncPerformed,
    cacheSyncWrittenCount,
    setResults,
    setLoading,
    setError,
    setLastQuery,
    setLastMode,
  };
});
