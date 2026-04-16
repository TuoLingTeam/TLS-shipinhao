import { useReviewStore } from "../stores/review";
import { useTauriInvoke } from "./useTauriInvoke";
import type { OrderMatchResult } from "../types/review";

export function useReview() {
  const store = useReviewStore();
  const { execute, loading } = useTauriInvoke<OrderMatchResult[]>("find_reviews");

  async function findReviews(days: number, startAt: string, endAt: string) {
    store.setLoading(true);
    store.setError(null);
    store.setLastQuery({ days, time_window: { start_at: startAt, end_at: endAt } });
    const result = await execute({ days, start_at: startAt, end_at: endAt });
    if (result) {
      store.setResults(result);
    } else {
      store.setError("查找失败");
    }
    store.setLoading(false);
  }

  return { findReviews, loading };
}
