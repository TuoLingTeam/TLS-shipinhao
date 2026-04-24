export const APP_NAME = "驼铃·视频小店差评处理";
export const APP_NAME_EN = "TLS-shipinhao";
export const AUTHOR_WECHAT = "TLS-801";
// __APP_VERSION__ 由 vite.config.ts 的 define 编译期注入（见 env.d.ts 的全局声明）
// 源头是 apps/ui/package.json 的 version，作为 Tauri API 未 ready 时的兜底值。
// 运行时首选通过 fetchAppVersion() 从 Tauri 读 tauri.conf.json 的 version，
// 保证与窗口标题、Rust 端 package_info 完全同源，永不漂移。
export const APP_VERSION = __APP_VERSION__;
export const WINDOW_TITLE = `${APP_NAME} ${APP_VERSION}`;

/**
 * 从 Tauri 运行时读取真实的应用版本号（源自 tauri.conf.json 的 version）。
 *
 * 调用失败时回退到编译期注入的 {@link APP_VERSION}，确保纯 web 预览、
 * 测试环境或 Tauri API 不可用场景下仍然有可用值。
 */
export async function fetchAppVersion(): Promise<string> {
  try {
    const { getVersion } = await import("@tauri-apps/api/app");
    return await getVersion();
  } catch {
    return APP_VERSION;
  }
}
