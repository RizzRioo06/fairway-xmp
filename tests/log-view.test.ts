import { test } from 'node:test';
import assert from 'node:assert/strict';
import { entriesSinceClear, logViewBaseline, serviceLogEntries } from '../src/log-view.ts';
import type { LogEntry, ServiceStatus } from '../src/types.ts';

const entry = (id: string, text: string): LogEntry => ({ id, text, source: 'php', file: 'php.log', severity: 'error', time: null });

test('cleared rows stay hidden on identical refreshes while later events remain visible', () => {
  const original = [entry('php.log:0:abc', 'old failure'), entry('mysql.log:0:def', 'old status')];
  const baseline = logViewBaseline(original);
  const incoming = entry('php.log:100:abc', 'old failure'); // Same text at a new file offset is a new event.
  assert.deepEqual(entriesSinceClear(structuredClone(original), baseline), []);
  assert.deepEqual(entriesSinceClear([...original, incoming], baseline), [incoming]);
  assert.deepEqual(entriesSinceClear(original, null), original); // A fresh app view retains file history.
  assert.equal(original.length, 2);
});

test('appended stack traces and changed service errors are visible after clearing', () => {
  const initial = entry('php.log:0:abc', 'Fatal error');
  const status = { id: 'mysql', message: 'Connection failed' } as ServiceStatus;
  const baseline = logViewBaseline([initial, ...serviceLogEntries([status])]);
  const continued = { ...initial, text: 'Fatal error\nStack trace:\n#0 new frame' };
  const changed = serviceLogEntries([{ ...status, message: 'Port is occupied' }]);
  assert.deepEqual(entriesSinceClear([continued, ...changed], baseline), [continued, ...changed]);
  assert.deepEqual(entriesSinceClear(serviceLogEntries([status]), baseline), []);
});

test('clearing captures all snapshot rows even when filters or the display cap hide them', () => {
  const snapshot = Array.from({ length: 900 }, (_, index) => entry(`id-${index}`, `event-${index}`));
  const baseline = logViewBaseline(snapshot);
  assert.deepEqual(entriesSinceClear(snapshot.slice(0, 240), baseline), []);
  assert.deepEqual(entriesSinceClear(snapshot.slice(240), baseline), []);
  const incoming = entry('new', 'fresh event');
  assert.deepEqual(entriesSinceClear([incoming], baseline), [incoming]);
  assert.deepEqual(entriesSinceClear([incoming], logViewBaseline([...snapshot, incoming])), []);
});
