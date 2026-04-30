/** 将未知类型的错误统一转成展示文案。 */
export function toErrorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
