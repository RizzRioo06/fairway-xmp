import assert from 'node:assert/strict';
import test from 'node:test';
import { needsClientRuntime } from '../src/postgresql/runtime-guidance.ts';
test('runtime recovery is limited to missing-client errors and does not mask authentication or TLS failures', () => {
  assert.equal(needsClientRuntime({ code: 'unavailable', text: 'Explicit runtime provisioning is required' }), true);
  assert.equal(needsClientRuntime({ code: 'unavailable', text: 'Runtime not provisioned' }), true);
  for (const value of [null, { code: 'connectionFailed', text: 'Password rejected' }, { code: 'unavailable', text: 'Remote TLS unavailable' }, { code: 'queryFailed', text: 'Runtime not provisioned' }]) assert.equal(needsClientRuntime(value), false);
});
