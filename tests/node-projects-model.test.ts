import assert from 'node:assert/strict';
import test from 'node:test';
import { activeRun, appendLogs, localUrl, mergeRun, nodeError } from '../src/node-projects/model.ts';
import type { NodeRunStatus, NodeLogRecord } from '../src/node-projects/types.ts';
const run: NodeRunStatus = { projectId: 'a'.repeat(32), runId: 'b'.repeat(32), revision: 2, state: 'running', createdAt: 100, startedAt: 101, endedAt: null, exitCode: null, signal: null, error: null, forcedCleanup: false, runtime: { nodePath: '/node/bin/node', version: 'v22.1.0' } };
test('run generations and revisions reject delayed events and require authority for equal timestamps', () => {
  assert.equal(mergeRun(run, { ...run, revision: 1, state: 'starting' }), run);
  assert.equal(mergeRun(run, { ...run, runId: 'c'.repeat(32), createdAt: 99 }), run);
  const next = { ...run, runId: 'c'.repeat(32), createdAt: 102, revision: 1 };
  assert.equal(mergeRun(run, next), next);
  const equal = { ...next, createdAt: 100 };
  assert.equal(mergeRun(run, equal), run);
  assert.equal(mergeRun(run, equal, true), equal);
  assert.equal(activeRun(run), true); assert.equal(activeRun({ ...run, state: 'exited' }), false);
});
test('local URLs reject credentials, normalized alternate hosts and invalid ports', () => {
  for (const url of ['', 'http://localhost:3000/', 'https://127.0.0.1/', 'http://[::1]:8080/?x=1']) assert.ok(localUrl(url), url);
  for (const url of ['javascript:alert(1)', 'http://localhost.evil/', 'http://user@localhost/', 'http://127.1/', 'http://2130706433/', 'http://localhost:0/', 'http://localhost:65536/', 'http://localhost/\n', 'http://localhost\\evil/']) assert.equal(localUrl(url), false, url);
});
test('log merges isolate runs, deduplicate cursors, preserve multiline Unicode and bound text and records', () => {
  const row: NodeLogRecord = { runId: run.runId, cursor: 1, time: 100, stream: 'stderr', text: '日本\nline 2\n<script>text</script>' };
  assert.deepEqual(appendLogs([row], [row, { ...row, runId: 'other', cursor: 2 }], run.runId, 1).records, [row]);
  const flood = Array.from({ length: 1200 }, (_, i) => ({ ...row, cursor: i + 2, text: '日'.repeat(1024) }));
  const bounded = appendLogs([row], flood, run.runId, 1);
  assert.ok(bounded.trimmed); assert.ok(bounded.records.length <= 1024);
  assert.ok(bounded.records.reduce((sum, row) => sum + new TextEncoder().encode(row.text).length, 0) <= 256 * 1024);
});
test('typed native failures keep useful recovery instructions', () => {
  assert.match(nodeError({ code: 'staleRun', message: 'Run changed.' }), /Refresh/);
  assert.match(nodeError({ code: 'requirementConflict', message: 'Mismatch.' }), /satisfying/);
  assert.equal(nodeError(new Error('network')), 'network');
});
