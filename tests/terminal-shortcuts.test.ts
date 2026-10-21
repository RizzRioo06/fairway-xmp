import { test } from 'node:test';
import assert from 'node:assert/strict';
import { terminalShortcut } from '../src/terminal-shortcuts.ts';
const key = (key: string, code: string, flags: Partial<KeyboardEvent> = {}) => ({ key, code, metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, ...flags });

test('ordinary control keys continue to readline and shell applications on every platform', () => {
  for (const macos of [true, false]) {
    for (const [value, code] of [['f', 'KeyF'], ['-', 'Minus'], ['=', 'Equal'], ['0', 'Digit0']]) {
      assert.equal(terminalShortcut(key(value, code, { ctrlKey: true }), macos), null);
    }
  }
});

test('platform shortcuts recognize shifted physical zoom keys without capturing Alt combinations', () => {
  for (const macos of [true, false]) {
    const flags = macos ? { metaKey: true } : { ctrlKey: true, shiftKey: true };
    for (const [value, code, action] of [['F', 'KeyF', 'find'], ['+', 'Equal', 'increase'], ['_', 'Minus', 'decrease'], [')', 'Digit0', 'reset']]) {
      assert.equal(terminalShortcut(key(value, code, flags), macos), action);
      assert.equal(terminalShortcut(key(value, code, { ...flags, altKey: true }), macos), null);
    }
  }
});
