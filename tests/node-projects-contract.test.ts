import assert from 'node:assert/strict';
import test from 'node:test';
import { isNodeProjectsError, isNodeRunStatus } from '../src/node-projects/types.ts';

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


test('run events keep process state separate from application readiness', () => {
  const run = { projectId: 'a'.repeat(32), runId: 'b'.repeat(32), revision: 1, state: 'starting', createdAt: 1, startedAt: null, endedAt: null, exitCode: null, signal: null, error: null, forcedCleanup: false, runtime: { nodePath: '/tools/bin/node', version: 'v22.1.0' } };
  assert.ok(isNodeRunStatus(run));
  assert.equal(isNodeRunStatus({ ...run, state: 'ready' }), false);
  assert.equal(isNodeRunStatus({ ...run, revision: NaN }), false);
  assert.equal(isNodeRunStatus(null), false);
  assert.equal(isNodeRunStatus({ ...run, runtime: { nodePath: 42, version: false } }), false);
  assert.equal(isNodeRunStatus({ ...run, runId: '../escape' }), false);
  assert.equal(isNodeRunStatus({ ...run, endedAt: undefined }), false);
  assert.equal(isNodeRunStatus({ ...run, error: 'untyped' }), false);
  for (const state of ['starting', 'running', 'stopping', 'stopped', 'exited', 'failed']) {
    assert.ok(isNodeRunStatus({ ...run, state }));
  }
  for (const code of ['confirmationRequired', 'projectBusy', 'staleRun', 'executionFailed', 'logUnavailable']) {
    assert.ok(isNodeProjectsError({ code, message: 'Controlled execution failure' }));
  }
});
