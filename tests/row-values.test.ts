import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fullCell, largeField, MAX_EDIT_BYTES, type ReadCell } from '../src/data/row-values.ts';

test('complete values use server character offsets, preserving Unicode, empty text and exact digits', async () => {
  const seen: number[] = [];
  const read: ReadCell = async (_, offset) => {
    seen.push(offset);
    return offset === 0 ? { value: '🟢é', offset, nextOffset: 2, total: 18, complete: false } : { value: '9007199254740993', offset, nextOffset: 18, total: 18, complete: true };
  };
  assert.equal(await fullCell(read, 'body', new AbortController().signal), '🟢é9007199254740993');
  assert.deepEqual(seen, [0, 2]);
  assert.equal(await fullCell(async () => ({ value: '', offset: 0, nextOffset: 0, total: 0, complete: true }), 'empty', new AbortController().signal), '');
});

test('no partial value escapes a stale read, cancellation, or invalid chunk', async () => {
  const controller = new AbortController();
  let calls = 0;
  const read: ReadCell = async (_, offset) => {
    if (++calls === 2) throw new Error('Row changed');
    return { value: 'preview', offset, nextOffset: 7, total: 20, complete: false };
  };
  await assert.rejects(fullCell(read, 'body', controller.signal), /Row changed/);
  await assert.rejects(fullCell(async () => { controller.abort(); return { value: 'partial', offset: 0, nextOffset: 7, total: 7, complete: true }; }, 'body', controller.signal), /abort/i);
  for (const chunk of [
    { value: null, offset: 0, nextOffset: 0, total: 0, complete: true },
    { value: '', offset: 0, nextOffset: 0, total: 1, complete: false },
    { value: 'x', offset: 1, nextOffset: 2, total: 2, complete: true },
    { value: 'x', offset: 0, nextOffset: 1, total: 2, complete: true },
  ]) await assert.rejects(fullCell(async () => chunk, 'body', new AbortController().signal), /completely/);
});

test('copy and form reads bound UTF-8 bytes and reject oversized values early', async () => {
  let calls = 0;
  await assert.rejects(fullCell(async () => { calls++; return { value: 'x', offset: 0, nextOffset: 1, total: MAX_EDIT_BYTES + 1, complete: false }; }, 'body', new AbortController().signal), /16 MiB/);
  assert.equal(calls, 1);
  await assert.rejects(fullCell(async () => ({ value: '🟢'.repeat(MAX_EDIT_BYTES / 4 + 1), offset: 0, nextOffset: MAX_EDIT_BYTES / 4 + 1, total: MAX_EDIT_BYTES / 4 + 1, complete: true }), 'body', new AbortController().signal), /16 MiB/);
});


test('large-field metadata only matches own column names', () => {
  assert.equal(largeField({}, 'constructor'), undefined);
  assert.equal(largeField({}, '__proto__'), undefined);
  assert.deepEqual(largeField(JSON.parse('{"__proto__":{"length":2000}}'), '__proto__'), { length: 2000 });
});
