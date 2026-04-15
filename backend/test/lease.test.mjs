import test from 'node:test';
import assert from 'node:assert/strict';

import { buildLeasePayload, computeLeaseWindow } from '../src/worker/lease.mjs';

test('computeLeaseWindow uses 24h renew window and 72h hard expiry', () => {
  const now = 1_700_000_000;
  const licenseExpiry = '2120-07-18T00:18:13+00:00';

  const lease = computeLeaseWindow({ nowEpochSeconds: now, licenseExpiresAt: licenseExpiry });

  assert.equal(lease.renewAfterEpochSeconds, now + 24 * 3600);
  assert.equal(lease.leaseExpiryEpochSeconds, now + 72 * 3600);
});

test('buildLeasePayload carries fixed policy and binding fields', () => {
  const payload = buildLeasePayload({
    record: {
      license_key: 'TLS-LEASE-TEST',
      expires_at: '2120-07-18T00:18:13+00:00',
      plan_days: 30,
      activated_at: '2026-03-10T00:18:13+00:00',
    },
    deviceId: '11223322eacf',
    issuedAtIso: '2026-03-10T00:18:13+00:00',
    nowEpochSeconds: 1_700_000_000,
    renewAfterEpochSeconds: 1_700_086_400,
    leaseExpiryEpochSeconds: 1_700_259_200,
    keysetVersion: 1,
  });

  assert.equal(payload.kind, 'license_lease');
  assert.equal(payload.device_id, '11223322eacf');
  assert.equal(payload.keyset_version, 1);
  assert.deepEqual(payload.task_policy, [
    'review_find',
    'review_full_scan',
    'quality_refund',
    'batch_delivery',
    'cache_manage',
  ]);
});
