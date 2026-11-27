import assert from 'node:assert/strict';
import test from 'node:test';
import { credentialStoreName } from '../src/postgresql/credential-store.ts';
test('credential labels identify each native vault without presenting unknown platforms as Keychain', () => {
  assert.equal(credentialStoreName('macos'), 'macOS Keychain');
  assert.equal(credentialStoreName('Windows'), 'Windows Credential Manager');
  assert.equal(credentialStoreName('linux'), 'Linux Secret Service');
  assert.equal(credentialStoreName('other'), 'system secure storage');
});
