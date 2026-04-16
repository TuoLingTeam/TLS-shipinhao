export function formatDate(iso: string): string {
  if (!iso) return "--";
  return new Date(iso).toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

export function formatCent(cent: number): string {
  return `¥${(cent / 100).toFixed(2)}`;
}
