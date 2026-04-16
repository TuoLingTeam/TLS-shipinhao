/**
 * Legacy compatibility shell.
 * 正式授权部署入口已迁移到 apps/license-worker（Rust Worker）。
 */

import ADMIN_HTML from "../admin/admin.html";

function jsonResponse(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "Access-Control-Allow-Origin": "*",
    },
  });
}

function htmlResponse(html) {
  return new Response(html, { headers: { "Content-Type": "text/html; charset=utf-8" } });
}

function compatibilityPayload(pathname) {
  return {
    success: false,
    message: "legacy_js_worker_retired_use_apps_license_worker",
    path: pathname,
    migration_target: "apps/license-worker",
  };
}

export default {
  async fetch(request) {
    const url = new URL(request.url);
    if (url.pathname === "/admin") {
      return htmlResponse(`${ADMIN_HTML}\n<!-- legacy_js_worker_retired_use_apps_license_worker -->`);
    }
    return jsonResponse(compatibilityPayload(url.pathname), 410);
  },
};
