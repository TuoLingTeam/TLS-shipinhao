/**
 * Playwright `addInitScript`：在任何 bundle 之前注入，模拟
 * `window.__TAURI_INTERNALS__.invoke`，使 Vite preview 下可跑涉及 IPC 的 E2E（Ext-3）。
 *
 * 仅用于测试；行为与真实 Tauri 无关，payload 以「前端不崩、类型大致兼容」为准。
 */
export const IPC_MOCK_BOOT = `
(function () {
  if (window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.__tlsE2eIpcMock) {
    return;
  }
  var health = {
    healthy: false,
    configured: false,
    has_biz_magic: false,
    last_checked_at: null,
    hint: null,
  };
  var cookieStatus = {
    configured: false,
    has_biz_magic: false,
    cookie_path: "",
    active_store: null,
    stores: [],
  };
  var orderStatus = {
    cached_order_count: 0,
    today_count: 0,
    yesterday_count: 0,
    last_7_days_count: 0,
    last_30_days_count: 0,
    today_latest_order_at: null,
    last_sync_at: null,
    coverage_start: null,
    coverage_end: null,
    coverage_complete: true,
    missing_segment_count: 0,
  };
  var orderCounts = {
    today_count: 0,
    yesterday_count: 0,
    last_7_days_count: 0,
    last_30_days_count: 0,
    today_latest_order_at: null,
  };
  var noUpdate = {
    app: "e2e",
    version: "0",
    build: 0,
    mandatory: false,
    platform: "e2e",
    download_url: "",
    tutorial_url: "",
    notes: [],
    has_update: false,
    raw_payload: {},
  };
  var emptyReview = {
    results: [],
    cache_warnings: [],
    cache_coverage_start: null,
    cache_coverage_end: null,
    cache_sync_performed: false,
    cache_sync_written_count: 0,
  };
  var orderSyncOk = {
    orders_saved: 0,
    cache_sync_performed: false,
    cache_coverage_start: null,
    cache_coverage_end: null,
    cache_warnings: [],
  };
  function migrationNoop() {
    return Promise.resolve({
      legacy_detected: false,
      cache_migrated: false,
      cookie_migrated: false,
      license_migrated: false,
      config_pointer_migrated: false,
      backup_dir: null,
      errors: [],
    });
  }
  function activateOk(args) {
    var key = (args && args.license_key) || "";
    return Promise.resolve({
      success: true,
      message: "e2e 模拟激活成功",
      license_state: "active",
      license_key: key || "E2E-MOCK-KEY",
      license_expires_at: "2099-01-01T00:00:00Z",
      lease_expires_at: "2099-01-04T00:00:00Z",
      last_verified_at: "2026-01-01T00:00:00Z",
      configured: true,
    });
  }
  function verifyOk(args) {
    var key = (args && args.license_key) || "";
    return Promise.resolve({
      success: true,
      message: "e2e 模拟刷新",
      license_state: "active",
      license_key: key || "E2E-MOCK-KEY",
      license_expires_at: "2099-01-01T00:00:00Z",
      lease_expires_at: "2099-01-04T00:00:00Z",
      last_verified_at: "2026-01-01T12:00:00Z",
    });
  }
  var handlers = {
    "plugin:app|version": function () {
      return Promise.resolve("5.0.3+e2e");
    },
    get_license_status: function () {
      return Promise.resolve({
        license_state: "not_found",
        success: true,
        configured: false,
      });
    },
    activate_license: activateOk,
    verify_license: verifyOk,
    get_cookie_health: function () {
      return Promise.resolve(health);
    },
    check_cookie_health: function () {
      return Promise.resolve(health);
    },
    get_cookie_status: function () {
      return Promise.resolve(cookieStatus);
    },
    get_order_cache_status: function () {
      return Promise.resolve(orderStatus);
    },
    get_order_cache_counts: function () {
      return Promise.resolve(orderCounts);
    },
    select_store: function (args) {
      var store = args && args.store_id
        ? { store_id: String(args.store_id), store_name: "e2e-store" }
        : { store_id: "e2e", store_name: "e2e-store" };
      return Promise.resolve({
        success: true,
        store: store,
        configured: false,
        has_biz_magic: false,
        cookie_path: "",
      });
    },
    check_for_update: function () {
      return Promise.resolve(noUpdate);
    },
    "plugin:event|listen": function () {
      return Promise.resolve(0);
    },
    "plugin:event|unlisten": function () {
      return Promise.resolve();
    },
    open_external_url: function () {
      return Promise.resolve();
    },
    close_cookie_login_window: function () {
      return Promise.resolve();
    },
    set_cookie: function () {
      return Promise.resolve();
    },
    open_cookie_login: function () {
      return Promise.resolve();
    },
    start_legacy_migration: migrationNoop,
    load_order_cache: function () {
      return Promise.resolve([]);
    },
    sync_recent_order_cache: function () {
      return Promise.resolve(orderSyncOk);
    },
    find_reviews: function () {
      return Promise.resolve(emptyReview);
    },
    find_quality_refund_orders: function () {
      return Promise.resolve(emptyReview);
    },
    cancel_batch_delivery: function () {
      return Promise.resolve(true);
    },
    update_delivery: function () {
      return Promise.resolve({
        order_id: "",
        success: true,
        previous_waybill: null,
        error_message: null,
      });
    },
    batch_delivery: function () {
      return Promise.resolve({
        total_count: 0,
        success_count: 0,
        failure_count: 0,
        stopped: false,
        fatal_error: null,
        steps: [],
      });
    },
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: function () {},
  };
  window.__TAURI_INTERNALS__ = {
    __tlsE2eIpcMock: true,
    transformCallback: function () {
      return 0;
    },
    convertFileSrc: function (p) {
      return String(p);
    },
    invoke: function (cmd, args) {
      var fn = handlers[cmd];
      if (fn) {
        return fn(args || {});
      }
      return Promise.reject(new Error("e2e unmocked invoke: " + cmd));
    },
  };
})();
`;
