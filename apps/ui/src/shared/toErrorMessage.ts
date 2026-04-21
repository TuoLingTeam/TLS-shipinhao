/**
 * 将未知类型的错误统一转成字符串展示。
 *
 * Tauri `invoke` 在失败时可能抛出 string（后端自定义 reject）或 Error（宿主侧异常），
 * 业务层历史上多处写着 `typeof e === "string" ? e : String(e)`；此处集中抽象便于一处演进
 * （例如未来想读 `Error.cause` / `Error.stack` / 做去敏处理时只改一个地方）。
 */
export function toErrorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return String(e);
}
