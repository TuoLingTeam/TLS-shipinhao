export const APP_NAME = "驼铃·视频小店差评处理";
export const APP_NAME_EN = "TLS-shipinhao";
export const AUTHOR_WECHAT = "TLS-801";
// 编译期版本来自 apps/ui/package.json；Tauri ready 后以运行时版本为准。
export const APP_VERSION = __APP_VERSION__;
export const WINDOW_TITLE = `${APP_NAME} ${APP_VERSION}`;

/** 读取 Tauri 版本号，失败时回退到编译期版本。 */
export async function fetchAppVersion(): Promise<string> {
  try {
    const { getVersion } = await import("@tauri-apps/api/app");
    return await getVersion();
  } catch {
    return APP_VERSION;
  }
}
