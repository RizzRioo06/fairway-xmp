import test from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from '../src/server.js';

test('health endpoint responds', async () => {
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    const response = await fetch(`http://127.0.0.1:${server.address().port}/health`);
    assert.deepEqual(await response.json(), { status: 'ok' });
  } finally { await new Promise(resolve => server.close(resolve)); }
});
