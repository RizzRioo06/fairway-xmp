import assert from 'node:assert/strict';
import test from 'node:test';
import { cellPreview, resultOutcome } from '../src/postgresql/result-presentation.ts';
test('table previews preserve exact short values and do not coerce large numeric strings', () => {
  for (const value of ['', '9007199254740993', '  padded\ntext\t', '{"big":9007199254740993}', '<script>text</script>']) assert.equal(cellPreview(value), value);
});
test('long previews are bounded without splitting Unicode; original cell values are unchanged', () => {
  const value = '😀日本'.repeat(100);
  assert.equal(cellPreview(value, 4), '😀日本😀…');
  assert.equal(value, '😀日本'.repeat(100));
  assert.equal(Array.from(cellPreview(value)).length, 161);
  assert.equal(cellPreview('😀日本', 3), '😀日本');
});
test('result summaries distinguish not-started, uncommitted and uncertain outcomes', () => {
  assert.equal(resultOutcome({ outcome: 'notStarted' }), 'Not started');
  assert.equal(resultOutcome({ outcome: 'notCommitted' }), 'Transaction not committed');
  assert.equal(resultOutcome({ outcome: 'committed' }), 'Changes committed');
  assert.match(resultOutcome({ outcome: 'unknown' }), /unknown.*verify before retrying/);
});
