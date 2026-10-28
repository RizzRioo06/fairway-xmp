import test from 'node:test';
import assert from 'node:assert/strict';
import { app } from '../src/app.js';

test('health endpoint responds', async () => {
  const server = app.listen(0, '127.0.0.1');
  await new Promise(resolve => server.once('listening', resolve));
  try {
    const response = await fetch(`http://127.0.0.1:${server.address().port}/health`);
    assert.deepEqual(await response.json(), { status: 'ok' });
  } finally { await new Promise(resolve => server.close(resolve)); }
});
