const DEFAULT_TASK_POLICY = Object.freeze([
  "review_find",
  "review_full_scan",
  "quality_refund",
  "batch_delivery",
  "cache_manage",
]);

export const LEASE_RENEWAL_HOURS = 24;
export const LEASE_HARD_EXPIRY_HOURS = 72;

function epochToISO(seconds) {
  return new Date(seconds * 1000).toISOString().replace(/\.\d{3}Z$/, "+00:00");
}

export function computeLeaseWindow(input, renewalHours = LEASE_RENEWAL_HOURS, hardExpiryHours = LEASE_HARD_EXPIRY_HOURS) {
  const nowEpochSeconds = typeof input === "number" ? input : Number(input?.nowEpochSeconds || 0);
  const renewAfterEpochSeconds = nowEpochSeconds + renewalHours * 3600;
  const leaseExpiryEpochSeconds = nowEpochSeconds + hardExpiryHours * 3600;
  return {
    issued_at: epochToISO(nowEpochSeconds),
    renew_after: epochToISO(renewAfterEpochSeconds),
    lease_expires_at: epochToISO(leaseExpiryEpochSeconds),
    renewAfterEpochSeconds,
    leaseExpiryEpochSeconds,
  };
}

export function buildLeasePayload({
  record,
  licenseKey,
  deviceId,
  licenseExpiresAt,
  licenseStatus = "active",
  nowEpochSeconds,
  taskPolicy = DEFAULT_TASK_POLICY,
  keysetVersion = 1,
  bindingVersion = 3,
  issuedAtIso,
  renewAfterEpochSeconds,
  leaseExpiryEpochSeconds,
}) {
  const resolvedLicenseKey = licenseKey || record?.license_key || "";
  const resolvedLicenseExpiresAt = licenseExpiresAt || record?.expires_at || "";
  const window = computeLeaseWindow({ nowEpochSeconds });
  const issuedAt = issuedAtIso || window.issued_at;
  const renewAfter = renewAfterEpochSeconds ? epochToISO(renewAfterEpochSeconds) : window.renew_after;
  const leaseExpiresAt = leaseExpiryEpochSeconds ? epochToISO(leaseExpiryEpochSeconds) : window.lease_expires_at;
  return {
    kind: "license_lease",
    issuer: "tls-license-backend",
    license_key: resolvedLicenseKey,
    device_id: deviceId,
    license_status: licenseStatus,
    license_expires_at: resolvedLicenseExpiresAt,
    lease_expires_at: leaseExpiresAt,
    renew_after: renewAfter,
    task_policy: [...taskPolicy],
    keyset_version: keysetVersion,
    binding_version: bindingVersion,
    issued_at: issuedAt,
    iat: nowEpochSeconds,
    exp: nowEpochSeconds + LEASE_HARD_EXPIRY_HOURS * 3600,
  };
}
