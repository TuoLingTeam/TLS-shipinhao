export type NavIconName = "dashboard" | "review" | "order" | "delivery" | "license" | "settings";
export type PageName = "dashboard" | "review" | "order" | "delivery" | "settings" | "license";
export type SettingsSectionId = "license" | "cookie" | "about";

export interface NavItem {
  path: string;
  label: string;
  icon: NavIconName;
  description: string;
}

export interface NavGroup {
  id: string;
  label: string;
  items: readonly NavItem[];
}

export interface PageMeta {
  eyebrow: string;
  title: string;
  description: string;
}

export interface SettingsSection {
  id: SettingsSectionId;
  label: string;
  description: string;
}

export const navGroups: readonly NavGroup[] = [
  {
    id: "workspace",
    label: "业务流程",
    items: [
      { path: "/", label: "仪表盘", icon: "dashboard", description: "总览状态" },
      { path: "/order", label: "订单管理", icon: "order", description: "缓存与检索" },
      { path: "/review", label: "评价管理", icon: "review", description: "检索与匹配" },
      { path: "/delivery", label: "发货管理", icon: "delivery", description: "单发与批量" },
    ],
  },
  {
    id: "system",
    label: "系统配置",
    items: [
      { path: "/settings", label: "软件设置", icon: "settings", description: "授权 / Cookie / 信息" },
    ],
  },
] as const;

export const pageMetaMap: Record<PageName, PageMeta> = {
  dashboard: {
    eyebrow: "TLS · OPERATIONS OVERVIEW",
    title: "仪表盘",
    description: "总览授权、Cookie、缓存与发货状态。",
  },
  review: {
    eyebrow: "TLS · REVIEW MATCHING",
    title: "评价管理",
    description: "差评 / 品退检索后可直接带入发货。",
  },
  order: {
    eyebrow: "TLS · ORDER CACHE",
    title: "订单管理",
    description: "维护近 30 天缓存，并支持本地快速检索。",
  },
  delivery: {
    eyebrow: "TLS · DELIVERY CONTROL",
    title: "发货管理",
    description: "单个发货、批量提交与进度追踪。",
  },
  settings: {
    eyebrow: "TLS · SETTINGS CENTER",
    title: "设置中心",
    description: "授权、Cookie 与版本信息集中管理。",
  },
  license: {
    eyebrow: "TLS · SETTINGS CENTER",
    title: "设置中心",
    description: "授权已并入设置中心。",
  },
};

export const settingsSections: readonly SettingsSection[] = [
  { id: "license", label: "授权", description: "激活与状态" },
  { id: "cookie", label: "Cookie", description: "登录与保存" },
  { id: "about", label: "应用", description: "品牌与提示" },
] as const;

export function isSettingsSection(value: unknown): value is SettingsSectionId {
  return value === "license" || value === "cookie" || value === "about";
}

export function buildSettingsLocation(section: SettingsSectionId = "cookie") {
  return {
    path: "/settings",
    query: { section },
  };
}
