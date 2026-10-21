import { test } from 'node:test';
import assert from 'node:assert/strict';
import { summarizeQueryResults } from '../src/data/query-results.ts';

function summaries(sql: string, counts: number[], sqlite = false) {
  return summarizeQueryResults(sql, { elapsedMs: 1, sets: counts.map(affected => ({ columns: [], rows: [], affected, truncated: false })) }, sqlite).sets.map(set => set.summary);
}

test('DML results name the change with correct singular, plural and zero counts', () => {
  assert.deepEqual(summaries('INSERT INTO t VALUES (1); UPDATE t SET x=2; DELETE FROM t;', [1, 2, 0]), ['1 row inserted', '2 rows updated', '0 rows deleted']);
  assert.deepEqual(summaries('CREATE TABLE t(x int); ALTER TABLE t ADD y int;', [0, 1]), ['Statement completed', 'Statement completed']);
  assert.deepEqual(summaries('UPDATE t SET x=1; UPDATE t SET x=2;', [0, 2]), ['0 rows updated', '2 rows updated']);
  assert.deepEqual(summaries('UPDATE t SET x=1; UPDATE t SET x=2;', [1, 2], true), ['1 row matched for update', '2 rows matched for update']);
});

test('comments, quoted semicolons and CTEs do not shift result labels', () => {
  assert.deepEqual(summaries("-- note\nINSERT INTO t VALUES ('x;y'); /* hidden DELETE */ WITH ids AS (SELECT 'UPDATE') UPDATE t SET x='z';", [1, 3]), ['1 row inserted', '3 rows updated']);
  assert.deepEqual(summaries('# note\nDELETE FROM t;', [1]), ['1 row deleted']);
  assert.deepEqual(summaries('WITH ids AS (SELECT 1) DELETE FROM t;', [4], true), ['4 rows deleted']);
});

test('upserts, REPLACE, procedures and mismatched result counts never claim inserts', () => {
  assert.deepEqual(summaries('INSERT INTO t VALUES (1) ON DUPLICATE KEY UPDATE x=1;', [2]), ['2 rows affected']);
  assert.deepEqual(summaries('INSERT INTO t VALUES (1) ON CONFLICT DO UPDATE SET x=1;', [1], true), ['1 row affected']);
  assert.deepEqual(summaries('INSERT OR REPLACE INTO t VALUES (1);', [1], true), ['1 row affected']);
  assert.deepEqual(summaries('CALL do_work(); INSERT INTO t VALUES (1);', [1, 2, 1]), ['1 row affected', '2 rows affected', '1 row affected']);
  assert.deepEqual(summaries('INSERT INTO t VALUES (1);', [1, 2]), ['1 row affected', '2 rows affected']);
  assert.deepEqual(summaries('/*! INSERT INTO t VALUES (1) */', [1]), ['1 row affected']);
});

test('result tables report returned rows and leave transport metadata intact', () => {
  const result = summarizeQueryResults('SELECT 1;', { elapsedMs: 12, sets: [{ columns: ['n'], rows: [['1']], affected: 1, truncated: true }] });
  assert.equal(result.sets[0].summary, '1 row returned');
  assert.equal(result.sets[0].truncated, true);
  assert.equal(result.elapsedMs, 12);
});
