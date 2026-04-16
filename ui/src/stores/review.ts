import { defineStore } from "pinia";
import { ref } from "vue";
import type { OrderMatchResult, ReviewQuery } from "../types/review";

export const useReviewStore = defineStore("review", () => {
  const results = ref<OrderMatchResult[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);
  const lastQuery = ref<ReviewQuery | null>(null);

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

  return { results, loading, error, lastQuery, setResults, setLoading, setError, setLastQuery };
});
