import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { isPostgresError } from '../src/postgresql/types.ts';
test('PostgreSQL discovery errors remain typed without accepting unrelated failures', () => {
  for (const code of ['busy', 'confirmationRequired', 'staleCandidate', 'unavailable']) {
    assert.ok(isPostgresError(JSON.parse(JSON.stringify({ code, message: 'Refresh discovery.' }))));
  }
  for (const value of [null, {}, new Error('error'), { code: 'saveFailed', message: 'error' }, { code: 'busy', message: 1 }]) assert.equal(isPostgresError(value), false);
});
test('Stage 1 bridge exposes discovery/version inspection and keeps managed/external control separate', () => {
  const api = readFileSync(new URL('../src/postgresql/api.ts', import.meta.url), 'utf8');
  const commands = [...api.matchAll(/invoke\('([^']+)'/g)].map(match => match[1]);
  assert.deepEqual(commands, ['postgres_runtime_discover', 'postgres_runtime_inspect']);
  const native = readFileSync(new URL('../src-tauri/src/desktop/postgresql_commands.rs', import.meta.url), 'utf8');
  for (const command of commands) assert.ok(native.includes(`fn ${command}(`));
  const rust = readFileSync(new URL('../src-tauri/src/postgresql.rs', import.meta.url), 'utf8');
  assert.ok(rust.includes('deny_unknown_fields'));
  assert.ok(rust.includes('pub confirmed: bool'));
});
