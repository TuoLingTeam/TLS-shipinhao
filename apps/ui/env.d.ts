/// <reference types="vite/client" />

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<
    Record<string, unknown>,
    Record<string, unknown>,
    unknown
  >;
  export default component;
}

// vite.config.ts 的 define 在编译期注入；源头是 apps/ui/package.json 的 version
declare const __APP_VERSION__: string;
