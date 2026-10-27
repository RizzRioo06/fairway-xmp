import assert from 'node:assert/strict';
import test from 'node:test';
import { isNodeProjectsError } from '../src/node-projects/types.ts';

test('native Node registry errors retain actionable codes across the bridge', () => {
  const unavailable: unknown = JSON.parse('{"code":"runtimeUnavailable","message":"Choose an installed runtime."}');
  assert.ok(isNodeProjectsError(unavailable));
  assert.equal(unavailable.code, 'runtimeUnavailable');
  assert.equal(unavailable.message, 'Choose an installed runtime.');
  assert.ok(isNodeProjectsError({ code: 'conflict', message: 'Refresh before saving.' }));
});

test('unrelated failures are not mistaken for typed Node registry errors', () => {
  for (const value of [null, undefined, 'missing', new Error('missing'), {}, { code: 'futureError', message: 'unknown' }, { code: 'saveFailed', message: 1 }]) {
    assert.equal(isNodeProjectsError(value), false);
  }
});


test('runtime preparation failures preserve distinct remediation codes', () => {
  for (const code of ['runtimeChanged', 'npmUnavailable', 'environmentUnavailable', 'requirementConflict', 'unsupportedPackageManager']) {
    const failure: unknown = JSON.parse(JSON.stringify({ code, message: 'Refresh or repair this selection.' }));
    assert.ok(isNodeProjectsError(failure));
    assert.equal(failure.code, code);
  }
});
